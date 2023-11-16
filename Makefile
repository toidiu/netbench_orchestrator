net_server_coord:
	cargo run --bin russula -- --protocol NetbenchServerCoordinator
net_server_worker:
	cargo run --bin russula -- --protocol NetbenchServerWorker


net_client_worker:
	cargo run --bin russula -- --protocol NetbenchClientWorker

