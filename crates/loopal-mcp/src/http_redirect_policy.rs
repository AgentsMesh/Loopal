use reqwest::redirect::Policy;

const MAX_REDIRECTS: usize = 10;

pub(crate) fn same_origin() -> Policy {
    Policy::custom(|attempt| {
        let Some(initial) = attempt.previous().first() else {
            return attempt.error("MCP HTTP redirect denied");
        };
        if attempt.previous().len() > MAX_REDIRECTS {
            return attempt.error("MCP HTTP redirect limit exceeded");
        }
        let next = attempt.url();
        if initial.scheme() == next.scheme()
            && initial.host_str() == next.host_str()
            && initial.port_or_known_default() == next.port_or_known_default()
        {
            attempt.follow()
        } else {
            attempt.error("MCP HTTP cross-origin redirect denied")
        }
    })
}
