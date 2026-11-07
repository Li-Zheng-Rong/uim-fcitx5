fn main() {
    pkg_config::Config::new()
        .atleast_version("1.9")
        .probe("uim")
        .expect("install UIM development headers and pkg-config metadata first");
}
