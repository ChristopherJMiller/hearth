// hearth-office uno — UNO component wiring
//
// This module contains the UNO service implementations that bridge LibreOffice's
// extension framework to the Nextcloud client modules. Each component implements
// one or more UNO interfaces (XDispatchProvider, XStatusbarController, etc.).
//
// Note: The actual UNO interface implementations depend on the rust_uno crate
// from LibreOffice 26.2. The current stubs provide the business logic and will
// be wired to UNO interfaces once rust_uno is available in the build.

pub mod comments_panel;
pub mod lock_status;
pub mod share_handler;
