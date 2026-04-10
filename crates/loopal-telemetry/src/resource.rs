//! OTel Resource construction with service metadata.

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;

/// Build the OTel Resource describing this service instance.
///
/// Session-specific attributes (session_id, agent_name) are carried as
/// span attributes on the `agent` span rather than on the Resource,
/// because the Resource is fixed at init time before sessions exist.
pub(crate) fn build_resource() -> Resource {
    Resource::builder()
        .with_attributes([
            KeyValue::new("service.name", "loopal"),
            KeyValue::new(
                "service.version",
                option_env!("LOOPAL_VERSION").unwrap_or("dev"),
            ),
            KeyValue::new("service.instance.id", std::process::id().to_string()),
        ])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Key;

    fn find_attr<'a>(
        resource: &'a Resource,
        name: &str,
    ) -> Option<(&'a Key, &'a opentelemetry::Value)> {
        resource.iter().find(|(k, _)| k.as_str() == name)
    }

    #[test]
    fn resource_has_service_name() {
        let resource = build_resource();
        let (_, value) =
            find_attr(&resource, "service.name").expect("resource must have service.name");
        assert_eq!(value.as_str(), "loopal");
    }

    #[test]
    fn resource_has_service_version() {
        let resource = build_resource();
        assert!(
            find_attr(&resource, "service.version").is_some(),
            "resource must have service.version"
        );
    }

    #[test]
    fn resource_has_instance_id() {
        let resource = build_resource();
        assert!(
            find_attr(&resource, "service.instance.id").is_some(),
            "resource must have service.instance.id"
        );
    }

    #[test]
    fn resource_does_not_have_session_id() {
        let resource = build_resource();
        assert!(
            find_attr(&resource, "loopal.session.id").is_none(),
            "session_id should be on spans, not resource"
        );
    }
}
