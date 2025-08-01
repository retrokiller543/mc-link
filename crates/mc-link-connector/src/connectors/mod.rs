pub mod ftp;
pub mod local;

pub use ftp::*;
pub use local::*;
use mc_link_core::{ProgressCallback, ServerConnector, ServerInfo};
use std::path::PathBuf;

pub enum Connector {
    Local(LocalConnector),
    Ftp(FtpConnector),
}

impl Connector {
    pub fn connection_type(&self) -> &'static str {
        match self {
            Connector::Local(_) => "Local",
            Connector::Ftp(_) => "FTP",
        }
    }
}

impl From<LocalConnector> for Connector {
    fn from(connector: LocalConnector) -> Self {
        Connector::Local(connector)
    }
}

impl From<FtpConnector> for Connector {
    fn from(connector: FtpConnector) -> Self {
        Connector::Ftp(connector)
    }
}

impl ServerConnector for Connector {
    fn connect(&mut self) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.connect().await,
                Connector::Ftp(connector) => connector.connect().await,
            }
        }
    }

    fn disconnect(&mut self) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.disconnect().await,
                Connector::Ftp(connector) => connector.disconnect().await,
            }
        }
    }

    fn is_connected(&self) -> impl Future<Output = bool> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.is_connected().await,
                Connector::Ftp(connector) => connector.is_connected().await,
            }
        }
    }

    fn get_server_info(&self) -> impl Future<Output = mc_link_core::Result<ServerInfo>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.get_server_info().await,
                Connector::Ftp(connector) => connector.get_server_info().await,
            }
        }
    }

    fn upload_file(
        &self,
        local_path: &PathBuf,
        remote_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => {
                    connector
                        .upload_file(local_path, remote_path, progress)
                        .await
                }
                Connector::Ftp(connector) => {
                    connector
                        .upload_file(local_path, remote_path, progress)
                        .await
                }
            }
        }
    }

    fn download_file(
        &self,
        remote_path: &PathBuf,
        local_path: &PathBuf,
        progress: Option<ProgressCallback>,
    ) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => {
                    connector
                        .download_file(remote_path, local_path, progress)
                        .await
                }
                Connector::Ftp(connector) => {
                    connector
                        .download_file(remote_path, local_path, progress)
                        .await
                }
            }
        }
    }

    fn list_files(
        &self,
        remote_path: &PathBuf,
    ) -> impl Future<Output = mc_link_core::Result<Vec<PathBuf>>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.list_files(remote_path).await,
                Connector::Ftp(connector) => connector.list_files(remote_path).await,
            }
        }
    }

    fn delete_file(
        &self,
        remote_path: &PathBuf,
    ) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.delete_file(remote_path).await,
                Connector::Ftp(connector) => connector.delete_file(remote_path).await,
            }
        }
    }

    fn create_directory(
        &self,
        remote_path: &PathBuf,
    ) -> impl Future<Output = mc_link_core::Result<()>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.create_directory(remote_path).await,
                Connector::Ftp(connector) => connector.create_directory(remote_path).await,
            }
        }
    }

    fn execute_command(
        &self,
        command: &str,
    ) -> impl Future<Output = mc_link_core::Result<String>> + Send {
        async move {
            match self {
                Connector::Local(connector) => connector.execute_command(command).await,
                Connector::Ftp(connector) => connector.execute_command(command).await,
            }
        }
    }
}
