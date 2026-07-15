use super::super::Method;

pub const WORKSPACE_LIST_DIRECTORY: Method = Method {
    name: "workspace/listDirectory",
};
pub const WORKSPACE_READ_FILE: Method = Method {
    name: "workspace/readFile",
};
pub const WORKSPACE_WRITE_FILE: Method = Method {
    name: "workspace/writeFile",
};
pub const WORKSPACE_SEARCH: Method = Method {
    name: "workspace/search",
};
pub const WORKSPACE_GIT_STATUS: Method = Method {
    name: "workspace/gitStatus",
};
pub const WORKSPACE_GIT_DIFF: Method = Method {
    name: "workspace/gitDiff",
};
pub const WORKSPACE_GIT_STAGE: Method = Method {
    name: "workspace/gitStage",
};
pub const WORKSPACE_GIT_UNSTAGE: Method = Method {
    name: "workspace/gitUnstage",
};
pub const WORKSPACE_LIST_WORKTREES: Method = Method {
    name: "workspace/listWorktrees",
};
pub const WORKSPACE_CREATE_WORKTREE: Method = Method {
    name: "workspace/createWorktree",
};
pub const WORKSPACE_REMOVE_WORKTREE: Method = Method {
    name: "workspace/removeWorktree",
};

pub const WORKSPACE_FILE_CHANGED: Method = Method {
    name: "workspace/fileChanged",
};
pub const WORKSPACE_GIT_CHANGED: Method = Method {
    name: "workspace/gitChanged",
};
pub const WORKSPACE_RESYNC_REQUIRED: Method = Method {
    name: "workspace/resyncRequired",
};
