#define_import_path mcrs_minecraft_client::volume

// An `#import` is resolved even behind an `#ifdef`, so the untinted lighting pass would wait for
// this module forever. Loaded only when nothing tints block light.
