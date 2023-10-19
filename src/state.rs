pub const STATE: State = State {
    version: "v1.0.1",

    // git
    // repo: "https://github.com/harrisonkaiser/s2n-quic.git",
    // branch: "netbench_sync",
    repo: "https://github.com/aws/s2n-quic.git",
    branch: "ak-netbench_sync",

    // aws
    log_bucket: "netbenchrunnerlogs",
    cf_url: "http://d2jusruq1ilhjs.cloudfront.net", // TODO use in code
    cloud_watch_group: "netbench_runner_logs",
    region: "us-west-1",
    vpc_region: "us-east-1",

    // orchestrator config
    shutdown_time: "7200", // 2 hrs
};

pub struct State {
    pub version: &'static str,
    // git
    pub repo: &'static str,
    pub branch: &'static str,

    // aws
    pub log_bucket: &'static str,
    pub cf_url: &'static str,
    pub cloud_watch_group: &'static str,
    pub region: &'static str,
    // TODO we shouldnt need two different regions. create infra in the single region
    pub vpc_region: &'static str,

    // orchestrator config
    pub shutdown_time: &'static str,
}

impl State {
    pub fn cf_url_with_id(&self, id: &str) -> String {
        format!("{}/{}", self.cf_url, id)
    }
}
