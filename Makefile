# -------------------- bin orchestrator
orch:
	RUST_LOG=none,orchestrator=debug cargo run --bin orchestrator

# -------------------- bin russula_cli
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=info cargo run --bin russula_cli -- \
					 --russula-port 7000 \
					 --testing=true \
					 netbench-server-coordinator

net_server_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=info cargo run --bin russula_cli -- \
					 --russula-port 7000 \
					 --testing=true \
					 netbench-server-worker \

net_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=info cargo run --bin russula_cli --  \
					 --russula-port 7001 \
					 --testing=true \
					 netbench-client-coordinator \

net_client_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=info cargo run --bin russula_cli -- \
					 --russula-port 7001 \
					 --testing=true \
					 netbench-client-worker \

report:
	s2n-netbench report netbench* -o report.json; xclip -sel c < report.json

# -------------------- lib russula
test_server:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- client --nocapture
