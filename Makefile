# -------------------- bin orchestrator
run_orchestrator:
	RUST_LOG=none,orchestrator=debug cargo run --bin orchestrator

# -------------------- bin russula_cli
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --russula-port 7000 \
					 --testing \
					 netbench-server-coordinator \
					 --driver unused

net_server_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --russula-port 7000 \
					 --testing \
					 netbench-server-worker \
					 --driver netbench-driver-s2n-quic-server

net_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli --  \
					 --russula-port 7001 \
					 --testing \
					 netbench-client-coordinator \
					 --driver unused

net_client_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- \
					 --russula-port 7001 \
					 --testing \
					 netbench-client-worker \
					 --driver netbench-driver-s2n-quic-client

report:
	s2n-netbench report netbench* -o report.json; xclip -sel c < report.json

# -------------------- lib russula
test_server:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- client --nocapture
