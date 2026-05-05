//! MetaHub-facing methods: cluster registration, cross-hub routing,
//! distributed spawn, topology aggregation.

use super::super::Method;

/// Sub-Hub registers with MetaHub after connecting.
pub const META_REGISTER: Method = Method {
    name: "meta/register",
};

/// Sub-Hub heartbeat to MetaHub (agent count, health).
pub const META_HEARTBEAT: Method = Method {
    name: "meta/heartbeat",
};

/// Cross-hub message routing (envelope forwarding).
pub const META_ROUTE: Method = Method { name: "meta/route" };

/// Cross-hub agent spawn delegation.
pub const META_SPAWN: Method = Method { name: "meta/spawn" };

/// List all connected Sub-Hubs.
pub const META_LIST_HUBS: Method = Method {
    name: "meta/list_hubs",
};

/// Global agent topology across all Sub-Hubs.
pub const META_TOPOLOGY: Method = Method {
    name: "meta/topology",
};
