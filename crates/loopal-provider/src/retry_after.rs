use chrono::{DateTime, NaiveDateTime, Utc};
use loopal_error::{MAX_RETRY_AFTER_MS, ProviderError};
use reqwest::header::{HeaderMap, RETRY_AFTER};

const DEFAULT_RATE_LIMIT_WAIT_MS: u64 = 30_000;
/// Parse both standard `Retry-After` forms: delta-seconds and an HTTP date.
/// Fractional seconds are accepted for local gateways and test doubles. Every
/// valid delay is capped so a malformed or hostile response cannot park the
/// session forever.
pub(crate) fn from_headers(headers: &HeaderMap) -> Option<u64> {
    from_headers_at(headers, Utc::now())
}

fn from_headers_at(headers: &HeaderMap, now: DateTime<Utc>) -> Option<u64> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<f64>() {
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        let millis = (seconds * 1000.0).ceil();
        return Some(cap_delay(millis));
    }

    let deadline = parse_http_date(value)?;
    let millis = deadline
        .signed_duration_since(now)
        .num_milliseconds()
        .max(0) as f64;
    Some(cap_delay(millis))
}

fn cap_delay(millis: f64) -> u64 {
    millis.min(MAX_RETRY_AFTER_MS as f64) as u64
}

fn parse_http_date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(value)
        .map(|date| date.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%a, %d %b %Y %H:%M:%S GMT")
                .map(|date| date.and_utc())
        })
        // Obsolete HTTP-date forms remain valid for recipients per RFC 9110.
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%A, %d-%b-%y %H:%M:%S GMT")
                .map(|date| date.and_utc())
        })
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%a %b %e %H:%M:%S %Y").map(|date| date.and_utc())
        })
        .ok()
}

/// Build the shared HTTP error shape after the adapter has safely read and
/// redacted the response body. Retry metadata is kept on 5xx errors as well as
/// 429 so the runtime can honor gateway-directed delays.
pub(crate) fn provider_error(
    status: u16,
    message: String,
    retry_after_ms: Option<u64>,
) -> ProviderError {
    if status == 429 {
        ProviderError::RateLimited {
            retry_after_ms: retry_after_ms.unwrap_or(DEFAULT_RATE_LIMIT_WAIT_MS),
        }
    } else {
        ProviderError::Api {
            status,
            message,
            retry_after_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use reqwest::header::HeaderValue;

    fn headers(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn parses_integral_and_fractional_seconds() {
        assert_eq!(from_headers(&headers("3")), Some(3_000));
        assert_eq!(from_headers(&headers("0.0015")), Some(2));
        assert_eq!(from_headers(&headers("1.25")), Some(1_250));
    }

    #[test]
    fn parses_standard_http_date_against_response_time() {
        let now = Utc.with_ymd_and_hms(2015, 10, 21, 7, 27, 55).unwrap();
        assert_eq!(
            from_headers_at(&headers("Wed, 21 Oct 2015 07:28:00 GMT"), now),
            Some(5_000)
        );
    }

    #[test]
    fn past_dates_are_immediate_and_large_values_are_capped() {
        let now = Utc.with_ymd_and_hms(2015, 10, 21, 7, 28, 5).unwrap();
        assert_eq!(
            from_headers_at(&headers("Wed, 21 Oct 2015 07:28:00 GMT"), now),
            Some(0)
        );
        assert_eq!(
            from_headers(&headers("999999999999")),
            Some(MAX_RETRY_AFTER_MS)
        );
    }

    #[test]
    fn rejects_invalid_delays() {
        for value in ["", "later", "-1", "NaN", "inf"] {
            assert_eq!(from_headers(&headers(value)), None, "value={value}");
        }
    }

    #[test]
    fn preserves_delay_on_every_retryable_server_status() {
        for status in [500, 502, 503, 504, 529] {
            let error = provider_error(status, "gateway failure".into(), Some(1_250));
            assert!(error.is_retryable(), "status={status}");
            assert_eq!(error.retry_after_ms(), Some(1_250), "status={status}");
        }
    }

    #[test]
    fn rate_limit_uses_default_only_when_header_is_absent() {
        assert!(matches!(
            provider_error(429, "limited".into(), None),
            ProviderError::RateLimited {
                retry_after_ms: DEFAULT_RATE_LIMIT_WAIT_MS
            }
        ));
        assert!(matches!(
            provider_error(429, "limited".into(), Some(17)),
            ProviderError::RateLimited { retry_after_ms: 17 }
        ));
    }
}
