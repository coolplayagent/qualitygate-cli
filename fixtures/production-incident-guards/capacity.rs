// Bounded model of one front-end's resource reservations and route publication.
use std::io;

const SERVICE_RESERVE: usize = 2;
const OLD: [u8; 2] = [10, 11];
const NEW: [u8; 3] = [20, 21, 22];
const CANDIDATE: [Option<u8>; 3] = [Some(20), Some(21), Some(22)];

struct Frontend {
    quota: usize,
    allocated: usize,
    peak: usize,
    active_routes: Vec<u8>,
}

impl Frontend {
    fn new(quota: usize) -> Self {
        Self {
            quota,
            allocated: OLD.len(),
            peak: OLD.len(),
            active_routes: OLD.to_vec(),
        }
    }

    fn serve(&self) -> Option<&[u8]> {
        let free = self.quota.saturating_sub(self.allocated);
        (free >= SERVICE_RESERVE).then_some(self.active_routes.as_slice())
    }

    fn refresh(&mut self, peers: &[Option<u8>], fixed: bool) -> bool {
        let free = self.quota.saturating_sub(self.allocated);
        if fixed && peers.len() > free.saturating_sub(SERVICE_RESERVE) {
            return false;
        }

        let allocated_before = self.allocated;
        let mut routes = Vec::new();
        for peer in peers {
            if self.allocated >= self.quota {
                break;
            }
            self.allocated += 1;
            self.peak = self.peak.max(self.allocated);
            let Some(route) = peer else {
                break;
            };
            routes.push(*route);
        }

        if routes.len() != peers.len() {
            if fixed {
                self.allocated = allocated_before;
            } else {
                self.active_routes = routes;
            }
            return false;
        }

        self.allocated -= self.active_routes.len();
        self.active_routes = routes;
        true
    }

    fn preserves_old(&self) -> bool {
        self.serve() == Some(OLD.as_slice()) && self.allocated == OLD.len()
    }
}

fn run_cases(fixed: bool) -> [(&'static str, bool); 6] {
    let mut exact = Frontend::new(7);
    let exact_boundary = exact.refresh(&CANDIDATE, fixed)
        && exact.serve() == Some(NEW.as_slice())
        && exact.peak == 5
        && exact.allocated == NEW.len();

    let mut removes_headroom = Frontend::new(6);
    let headroom = !removes_headroom.refresh(&CANDIDATE, fixed)
        && removes_headroom.preserves_old()
        && removes_headroom.peak == OLD.len();

    let mut exhausts_quota = Frontend::new(4);
    let exhaustion = !exhausts_quota.refresh(&CANDIDATE, fixed)
        && exhausts_quota.preserves_old()
        && exhausts_quota.peak == OLD.len();

    let mut incomplete_metadata = Frontend::new(7);
    let mut retains_old = true;
    for _ in 0..3 {
        retains_old &= !incomplete_metadata.refresh(&[Some(20), None, Some(22)], fixed)
            && incomplete_metadata.preserves_old()
            && incomplete_metadata.peak == 4;
    }
    let retry_after_metadata_failure = incomplete_metadata.refresh(&CANDIDATE, fixed)
        && incomplete_metadata.serve() == Some(NEW.as_slice())
        && incomplete_metadata.allocated == NEW.len();

    let mut increased_quota = Frontend::new(6);
    let rejected = !increased_quota.refresh(&CANDIDATE, fixed);
    increased_quota.quota = 7;
    let retries_after_increase = rejected
        && increased_quota.refresh(&CANDIDATE, fixed)
        && increased_quota.serve() == Some(NEW.as_slice());

    [
        ("exact-boundary-keeps-serving", exact_boundary),
        ("reject-before-consuming-service-reserve", headroom),
        ("quota-exhaustion-preserves-route", exhaustion),
        (
            "partial-metadata-keeps-route-and-releases-slots",
            retains_old,
        ),
        (
            "metadata-retry-publishes-and-retires-old",
            retry_after_metadata_failure,
        ),
        ("increased-quota-allows-retry", retries_after_increase),
    ]
}

fn main() -> io::Result<()> {
    let fixed = match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("incident-capacity 1");
            return Ok(());
        }
        Some("before") => false,
        Some("after") => true,
        _ => return Err(io::Error::other("expected before, after or --version")),
    };

    let cases = run_cases(fixed);
    let failures = cases.iter().filter(|(_, passed)| !passed).count();
    let mut report = format!(
        "<testsuite name='capacity' tests='{}' failures='{failures}'>",
        cases.len()
    );
    for (name, passed) in cases {
        let outcome = if passed {
            ""
        } else {
            "<failure message='capacity invariant failed'/>"
        };
        report.push_str(&format!("<testcase name='{name}'>{outcome}</testcase>"));
    }
    report.push_str("</testsuite>");
    std::fs::create_dir_all("target")?;
    std::fs::write("target/capacity.xml", report)
}
