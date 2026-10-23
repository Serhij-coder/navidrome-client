use super::*;

#[test]
fn a_pipe_owned_by_another_process_is_rejected_before_protocol_io() {
    let directory = tempfile::tempdir().unwrap();
    let path = endpoint(&directory).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();
    let server = {
        let _guard = runtime.enter();
        tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(true)
            .create(&path)
            .unwrap()
    };
    // PID 0 cannot own this test's server; no IPC command should be sent to it.
    let connector = Connector::new().unwrap();
    let error = match connector.connect(&path, 0) {
        Ok(_) => panic!("foreign server was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    drop(server);
    assert!(connector.connect(&path, std::process::id()).is_err());
}
