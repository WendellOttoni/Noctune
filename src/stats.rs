use sysinfo::{Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

pub struct SystemStats {
    sys: System,
    networks: Networks,
    pid: Pid,
    pub cpu_pct: f32,
    pub ram_mb: u64,
    pub net_down_kbps: f64,
}

impl SystemStats {
    pub fn new() -> Self {
        let pid = Pid::from_u32(std::process::id());
        let sys = System::new_with_specifics(
            RefreshKind::new().with_processes(
                ProcessRefreshKind::new().with_cpu().with_memory(),
            ),
        );
        let networks = Networks::new_with_refreshed_list();
        Self { sys, networks, pid, cpu_pct: 0.0, ram_mb: 0, net_down_kbps: 0.0 }
    }

    /// Call once per second from the tick loop.
    pub fn refresh(&mut self) {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::new().with_cpu().with_memory(),
        );
        if let Some(proc) = self.sys.process(self.pid) {
            self.cpu_pct = proc.cpu_usage();
            self.ram_mb = proc.memory() / 1_048_576;
        }

        self.networks.refresh();
        let bytes: u64 = self.networks.values().map(|n| n.received()).sum();
        self.net_down_kbps = bytes as f64 / 1024.0;
    }
}
