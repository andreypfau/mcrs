# Security

mcrs is in early development and is not meant for the public internet.

- The server has no authentication. Online mode and encryption are not implemented (issue #82), so anyone can join under any name. By default it listens on `0.0.0.0:25565` (`crates/mcrs_minecraft_engine_server/src/server_network_plugin.rs`) and new players start in creative mode.
- Do not open the port to the internet. Run the server on a local machine or behind a firewall that you control.

## Report a vulnerability

Use GitHub Security Advisories of this repository: the "Report a vulnerability" button on the Security tab, https://github.com/andreypfau/mcrs/security/advisories/new. Please do not open a public issue with the details. If the button is not available, open an issue that says only "I have a security report" and no details, and the owner will contact you.
