use crate::models::ServiceStatus;

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Key(crossterm::event::KeyEvent),
    #[allow(dead_code)]
    Mouse(crossterm::event::MouseEvent),
    ServiceLog(String, String), // Service Name, Log Line
    ServiceStatus(String, ServiceStatus), // Service Name, New Status
    #[allow(dead_code)]
    Quit,
}
