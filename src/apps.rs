use crate::models::Image;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum App {
    Docker,
    Nginx,
    Caddy,
}

#[derive(Debug, Default)]
pub struct CloudInit {
    packages: Vec<&'static str>,
    runcmd: Vec<String>,
}

impl CloudInit {
    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.packages.is_empty() {
            out.push_str("packages:\n");
            for p in &self.packages {
                out.push_str(&format!("  - {p}\n"));
            }
        }
        if !self.runcmd.is_empty() {
            out.push_str("runcmd:\n");
            for c in &self.runcmd {
                out.push_str(&format!("  - {c}\n"));
            }
        }
        out
    }
}

pub fn cloud_init(apps: &[App], image: Image, user: &str) -> anyhow::Result<CloudInit> {
    let mut ci = CloudInit::default();
    // ubuntu/kali are apt based, centos is dnf
    let apt = image != Image::Centos10;
    for app in apps {
        match app {
            App::Nginx => ci.packages.push("nginx"),
            App::Docker => {
                if apt {
                    ci.packages.push("docker.io");
                } else {
                    ci.runcmd
                        .push("curl -fsSL https://get.docker.com | sh".into());
                }
                ci.runcmd.push(format!("usermod -aG docker {user}"));
            }
            App::Caddy => {
                anyhow::ensure!(
                    image != Image::Ubuntu22,
                    "caddy is not available for ubuntu22"
                );
                if apt {
                    ci.packages.push("caddy");
                } else {
                    ci.runcmd.push("dnf copr enable -y @caddy/caddy".into());
                    ci.runcmd.push("dnf install -y caddy".into());
                }
            }
        }
    }
    Ok(ci)
}
