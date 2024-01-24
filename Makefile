# -------------------- bin orchestrator
run_orchestrator:
	RUST_LOG=none,orchestrator::russula=info,orchestrator=debug cargo run --bin orchestrator

# -------------------- bin russula_cli
#
# russula_cli --russula-port {} \
# 	netbench-server-worker \
# 	--peer-list {peer_sock_addr} \
# 	--driver {netbench_driver} \
# 	--scenario {}
#
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 netbench-server-coordinator \
					 --worker-addrs 0.0.0.0:7000 \

net_server_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 netbench-server-worker \
					 --russula-port 7000 \
					 --testing \
					 --driver testing
					 # --driver netbench-driver-s2n-quic-server

net_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli --  \
					 netbench-client-coordinator \
					 --worker-addrs 0.0.0.0:7001 \

net_client_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 netbench-client-worker \
					 --russula-port 7001 \
					 --testing \
					 --driver testing \
					 # --driver netbench-driver-s2n-quic-client

report:
	s2n-netbench report netbench* -o report.json; xclip -sel c < report.json

# -------------------- lib russula
test_server:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- client --nocapture
