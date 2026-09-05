#include <stdint.h>

/*
 * Deliberately dependency-free PE entry point.
 * Wisp will eventually call this directly after mapping the image.
 */
__attribute__((noinline)) int wisp_entry(void) {
    volatile uint64_t value = 40;
    value += 2;
    return (int)value;
}
