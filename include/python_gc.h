#ifndef PYTHON_GC_H
#define PYTHON_GC_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>


typedef enum {
    GC_SUCCESS = 0,
    GC_ERROR_ALREADY_TRACKED = -1,
    GC_ERROR_NOT_TRACKED = -2,
    GC_ERROR_COLLECTION_IN_PROGRESS = -3,
    GC_ERROR_INVALID_GENERATION = -4,
    GC_ERROR_INTERNAL = -5,
} gc_return_code_t;


typedef struct {
    int32_t total_tracked;
    int32_t generation_counts[3];
    int32_t uncollectable;
} gc_stats_t;



/**
 * Initialize the global garbage collector
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_init(void);

/**
 * Clean up the global garbage collector
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_cleanup(void);

/**
 * Enable automatic garbage collection
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_enable(void);

/**
 * Disable automatic garbage collection
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_disable(void);

/**
 * Check if automatic collection is enabled
 * @return 1 if enabled, 0 if disabled
 */
int32_t py_gc_is_enabled(void);



/**
 * Track an object for garbage collection
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_track(void* obj_ptr);

/**
 * Stop tracking an object
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_untrack(void* obj_ptr);

/**
 * Check if an object is tracked by the garbage collector
 * @param obj_ptr Pointer to the Python object
 * @return 1 if tracked, 0 if not tracked
 */
int32_t py_gc_is_tracked(void* obj_ptr);



/**
 * Perform garbage collection on a specific generation
 * @param generation Generation number (0, 1, or 2)
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_collect_generation(int32_t generation);

/**
 * Perform a full garbage collection (all generations)
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_collect(void);

/**
 * Check if any generation needs collection
 * @return 1 if collection is needed, 0 otherwise
 */
int32_t py_gc_needs_collection(void);

/**
 * Force collection if thresholds are exceeded
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_collect_if_needed(void);



/**
 * Get the number of tracked objects
 * @return Number of tracked objects
 */
int32_t py_gc_get_count(void);

/**
 * Get the count of objects in a specific generation
 * @param generation Generation number (0, 1, or 2)
 * @return Number of objects in the generation, or -1 on error
 */
int32_t py_gc_get_generation_count(int32_t generation);

/**
 * Set collection threshold for a generation
 * @param generation Generation number (0, 1, or 2)
 * @param threshold Threshold value
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_threshold(int32_t generation, int32_t threshold);

/**
 * Get collection threshold for a generation
 * @param generation Generation number (0, 1, or 2)
 * @return Threshold value, or -1 on error
 */
int32_t py_gc_get_threshold(int32_t generation);

/**
 * Set debug flags
 * @param flags Debug flags
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_debug(int32_t flags);

/**
 * Get statistics about the garbage collector
 * @param stats Pointer to stats structure to fill
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_stats(gc_stats_t* stats);

/**
 * Get the count of uncollectable objects
 * @return Number of uncollectable objects
 */
int32_t py_gc_get_uncollectable_count(void);

/**
 * Clear the uncollectable list
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_clear_uncollectable(void);



/**
 * Check if the GC is properly initialized
 * @return 1 if initialized, 0 if not
 */
int32_t py_gc_is_initialized(void);

/**
 * Get the current GC state as a string (for debugging)
 * @param buffer Buffer to store the state string
 * @param buffer_size Size of the buffer
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_state_string(char* buffer, size_t buffer_size);

#ifdef __cplusplus
}
#endif

#endif 