# -------------------- bin orchestrator
run_orchestrator:
	RUST_LOG=none,orchestrator::russula=info,orchestrator=debug cargo run --bin orchestrator

# -------------------- test russula_cli with netbench
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-coordinator \
					 --worker-addrs 0.0.0.0:7000 \
					 0.0.0.0:8000 \

net_server_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-worker \
					 --russula-port 7000 \
					 --netbench-path ~/projects/s2n-netbench/target/release \
					 --driver s2n-netbench-driver-server-s2n-quic \
					 --scenario request_response_incast.json \
					 --netbench-port 7001 \

net_server_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 2s \
					 netbench-server-worker \
					 --russula-port 8000 \
					 --netbench-path ~/projects/s2n-netbench/target/release \
					 --driver s2n-netbench-driver-server-s2n-quic \
					 --scenario request_response_incast.json \
					 --netbench-port 8001 \


# -------------------- test russula_cli
test_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-coordinator \
					 --worker-addrs 0.0.0.0:7000 0.0.0.0:7001 \

test_server_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-worker \
					 --russula-port 7000 \
					 --testing \
					 --driver unused

test_server_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-server-worker \
					 --russula-port 7001 \
					 --testing \
					 --driver unused

test_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli --  \
					 --poll-delay 1s \
					 netbench-client-coordinator \
					 --worker-addrs 0.0.0.0:8000 0.0.0.0:8001 \

test_client_worker1:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-client-worker \
					 --russula-port 8000 \
					 --testing \
					 --driver unused \

test_client_worker2:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --poll-delay 1s \
					 netbench-client-worker \
					 --russula-port 8001 \
					 --testing \
					 --driver unused \

# -------------------- test russula
test_server:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- client --nocapture

# -------------------- util to generate netbench report
report:
	s2n-netbench report net_data_* -o report.json; xclip -sel c < report.json
