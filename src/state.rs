pub const STATE: State = State {
    version: "v1.0.0",
    log_bucket: "netbenchrunnerlogs",
    cf_url: "http://d2jusruq1ilhjs.cloudfront.net/", // TODO use in code
    // harrison
    // repo: "https://github.com/harrisonkaiser/s2n-quic.git",
    // branch: "netbench_sync",
    // aws
    repo: "https://github.com/aws/s2n-quic.git",
    branch: "ak-netbench_sync",
    shutdown_time: "7200", // 2 hrs
    cloud_watch_group: "netbench_runner_logs",
};

pub struct State {
    pub version: &'static str,
    pub log_bucket: &'static str,
    pub cf_url: &'static str,
    pub repo: &'static str,
    pub branch: &'static str,
    pub shutdown_time: &'static str,
    pub cloud_watch_group: &'static str,
}
