use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Killed,
}

impl TaskStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, TaskStatus::Running)
    }

    // reason: BgTaskStatus (protocol) 和 TaskStatus (internal) 必须保持 variant
    // 一一对应。新增 variant 时两边同步修改，否则 to_bg 会漏 arm 触发编译错误。
    pub fn to_bg(self) -> loopal_protocol::BgTaskStatus {
        match self {
            TaskStatus::Running => loopal_protocol::BgTaskStatus::Running,
            TaskStatus::Completed => loopal_protocol::BgTaskStatus::Completed,
            TaskStatus::Failed => loopal_protocol::BgTaskStatus::Failed,
            TaskStatus::Killed => loopal_protocol::BgTaskStatus::Killed,
        }
    }
}

pub enum ControlSignal {
    Stop { ack: oneshot::Sender<StopOutcome> },
}

#[derive(Debug)]
pub enum StopOutcome {
    Killed { exit_code: Option<i32> },
    KillFailed(String),
}

#[derive(Debug)]
pub enum StoreError {
    NotFound,
    AlreadyTerminal {
        status: TaskStatus,
        exit_code: Option<i32>,
    },
    // reason: race 兜底 — try_send 在 monitor task 已退出且 control_rx 已 drop
    // 时返回此错误。理论上 send_stop 入口已经检查 is_terminal()，到达此分支
    // 意味着检查后 monitor 在 lock 释放和 try_send 之间退出。保留作为可观察
    // 的 race signal，方便排查；调用方应视作 AlreadyTerminal 处理。
    ChannelClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    Running,
    Terminal,
    All,
}
