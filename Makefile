# -------------------- bin orchestrator
orch:
	RUST_LOG=debug cargo run --bin orchestrator

# -------------------- bin russula_runner
net_server_coord:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchServerCoordinator
net_server_worker:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchServerWorker

net_client_coord:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchClientCoordinator
net_client_worker:
	RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchClientWorker

# -------------------- lib russula
test_server:
	RUST_LOG=debug cargo test --bin orchestrator -- server --nocapture
test_client:
	RUST_LOG=debug cargo test --bin orchestrator -- client --nocapture
