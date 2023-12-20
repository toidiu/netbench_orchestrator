# -------------------- bin orchestrator
orch:
	RUST_LOG=none,orchestrator=debug cargo run --bin orchestrator

# -------------------- bin russula_cli
net_server_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- --protocol NetbenchServerCoordinator
net_server_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- --protocol NetbenchServerWorker --peer-list 127.0.0.1:4433
	# ./target/debug/russula_cli --protocol NetbenchServerWorker --peer-list 127.0.0.1:4433

net_client_coord:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- --protocol NetbenchClientCoordinator --ip 127.0.0.1
net_client_worker:
	RUST_LOG=none,orchestrator=debug,russula_cli=debug cargo run --bin russula_cli -- --protocol NetbenchClientWorker --ip 127.0.0.1 --peer-list 127.0.0.1:4433

# -------------------- lib russula
test_server:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=none,orchestrator=debug cargo test --bin orchestrator -- client --nocapture
