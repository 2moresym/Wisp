#include <stdint.h>

/* Keep the C boundary tiny: graphics backends stay native and are called from Rust only at ownership boundaries. */
uint32_t wisp_glue_abi_version(void) { return 1; }
