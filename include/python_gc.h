#ifndef PYTHON_GC_H
#define PYTHON_GC_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

// Return codes for GC operations
typedef enum {
    GC_SUCCESS = 0,
    GC_ERROR_ALREADY_TRACKED = -1,
    GC_ERROR_NOT_TRACKED = -2,
    GC_ERROR_COLLECTION_IN_PROGRESS = -3,
    GC_ERROR_INVALID_GENERATION = -4,
    GC_ERROR_INTERNAL = -5,
} gc_return_code_t;

// GC statistics structure
typedef struct {
    int32_t total_tracked;
    int32_t generation_counts[3];
    int32_t uncollectable;
} gc_stats_t;

// Core GC Management Functions

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
 * Check if GC is initialized
 * @return 1 if initialized, 0 if not
 */
int32_t py_gc_is_initialized(void);

// Automatic Tracking Functions

/**
 * Enable automatic tracking of Python objects
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_enable_automatic_tracking(void);

/**
 * Disable automatic tracking of Python objects
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_disable_automatic_tracking(void);

/**
 * Check if automatic tracking is enabled
 * @return 1 if enabled, 0 if disabled
 */
int32_t py_gc_is_automatic_tracking_enabled(void);

// Python Object Hooks

/**
 * Hook for Python object creation (called by Python when objects are created)
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_object_created(void* obj_ptr);

/**
 * Hook for Python object destruction (called by Python when objects are destroyed)
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_object_destroyed(void* obj_ptr);

/**
 * Hook for Python reference count changes
 * @param obj_ptr Pointer to the Python object
 * @param old_count Previous reference count
 * @param new_count New reference count
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_refcount_changed(void* obj_ptr, int32_t old_count, int32_t new_count);

// Manual Object Tracking

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

// Python GC Module Compatibility

/**
 * Track a Python object (Python gc module compatibility)
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_track_python(void* obj_ptr);

/**
 * Untrack a Python object (Python gc module compatibility)
 * @param obj_ptr Pointer to the Python object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_untrack_python(void* obj_ptr);

/**
 * Check if a Python object is tracked (Python gc module compatibility)
 * @param obj_ptr Pointer to the Python object
 * @return 1 if tracked, 0 if not tracked
 */
int32_t py_gc_is_tracked_python(void* obj_ptr);

// Reference Counting Functions

/**
 * Get Python object reference count
 * @param obj_ptr Pointer to the Python object
 * @return Reference count, or 0 if object is NULL
 */
int32_t py_gc_get_refcount(void* obj_ptr);

/**
 * Set Python object reference count
 * @param obj_ptr Pointer to the Python object
 * @param refcount New reference count
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_refcount(void* obj_ptr, int32_t refcount);

// Collection Functions

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
 * Check if collection is needed
 * @return 1 if collection is needed, 0 if not
 */
int32_t py_gc_needs_collection(void);

/**
 * Collect if thresholds are exceeded
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_collect_if_needed(void);

// Statistics and Information

/**
 * Get garbage collection statistics
 * @param stats Pointer to GCStats structure to fill
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_stats(gc_stats_t* stats);

/**
 * Get the number of tracked objects
 * @return Number of tracked objects
 */
int32_t py_gc_get_count(void);

/**
 * Get the number of objects in a specific generation
 * @param generation Generation number (0, 1, or 2)
 * @return Number of objects in generation, or -1 if invalid generation
 */
int32_t py_gc_get_generation_count(int32_t generation);

/**
 * Get collection counts (Python gc.get_count() compatibility)
 * @return Pointer to array of 3 integers [gen0, gen1, gen2], or NULL on error
 */
int32_t* py_gc_get_collection_counts(void);

/**
 * Free collection counts array
 * @param counts Pointer to collection counts array
 */
void py_gc_free_collection_counts(int32_t* counts);

/**
 * Get the number of uncollectable objects
 * @return Number of uncollectable objects
 */
int32_t py_gc_get_uncollectable_count(void);

/**
 * Get the number of objects in the registry
 * @return Number of objects in registry
 */
int32_t py_gc_get_registry_count(void);

// Threshold Management

/**
 * Set threshold for a generation
 * @param generation Generation number (0, 1, or 2)
 * @param threshold New threshold value
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_threshold(int32_t generation, int32_t threshold);

/**
 * Get threshold for a generation
 * @param generation Generation number (0, 1, or 2)
 * @return Threshold value, or -1 if invalid generation
 */
int32_t py_gc_get_threshold(int32_t generation);

// Debug and State Functions

/**
 * Set debug flags
 * @param flags Debug flags to set
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_debug(int32_t flags);

/**
 * Set debug flags (Python gc module compatibility)
 * @param flags Debug flags to set
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_debug_flags(int32_t flags);

/**
 * Get debug flags
 * @return Current debug flags
 */
int32_t py_gc_get_debug_flags(void);

/**
 * Get a string representation of the GC state
 * @param buffer Buffer to write state string to
 * @param buffer_size Size of the buffer
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_state_string(char* buffer, size_t buffer_size);

/**
 * Get information about a tracked object
 * @param obj_ptr Pointer to the tracked object
 * @param buffer Buffer to write object info to
 * @param buffer_size Size of the buffer
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_tracked_info(void* obj_ptr, char* buffer, size_t buffer_size);

/**
 * Get object type name
 * @param obj_ptr Pointer to the object
 * @param buffer Buffer to write type name to
 * @param buffer_size Size of the buffer
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_get_object_type_name(void* obj_ptr, char* buffer, size_t buffer_size);

/**
 * Get object size
 * @param obj_ptr Pointer to the object
 * @return Object size in bytes, or 0 if not tracked
 */
int32_t py_gc_get_object_size(void* obj_ptr);

// Finalizer Management

/**
 * Check if object has finalizer
 * @param obj_ptr Pointer to the object
 * @return 1 if object has finalizer, 0 if not
 */
int32_t py_gc_has_finalizer(void* obj_ptr);

/**
 * Set object finalizer
 * @param obj_ptr Pointer to the object
 * @param has_finalizer 1 to set finalizer, 0 to clear
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_finalizer(void* obj_ptr, int32_t has_finalizer);

// Python GC Module Compatibility (Fully Implemented)

/**
 * Get all tracked objects (Python gc.get_objects() compatibility)
 * @return Pointer to array of tracked objects, or NULL
 */
void* py_gc_get_objects(void);

/**
 * Get objects that refer to the given object (Python gc.get_referrers() compatibility)
 * @param obj_ptr Pointer to the object
 * @return Pointer to array of referrers, or NULL
 */
void* py_gc_get_referrers(void* obj_ptr);

/**
 * Get objects that the given object refers to (Python gc.get_referents() compatibility)
 * @param obj_ptr Pointer to the object
 * @return Pointer to array of referents, or NULL
 */
void* py_gc_get_referents(void* obj_ptr);

/**
 * Get garbage (uncollectable objects) (Python gc.get_garbage() compatibility)
 * @return Pointer to array of uncollectable objects, or NULL
 */
void* py_gc_get_garbage(void);

/**
 * Set garbage list (Python gc module compatibility)
 * @param garbage_list Pointer to array of garbage objects
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_set_garbage(void* garbage_list);

// Reference Management Functions

/**
 * Add a reference from one object to another
 * @param from_obj Pointer to the referring object
 * @param to_obj Pointer to the referenced object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_add_reference(void* from_obj, void* to_obj);

/**
 * Remove a reference from one object to another
 * @param from_obj Pointer to the referring object
 * @param to_obj Pointer to the referenced object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_remove_reference(void* from_obj, void* to_obj);

/**
 * Mark an object as uncollectable
 * @param obj_ptr Pointer to the object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_mark_uncollectable(void* obj_ptr);

/**
 * Unmark an object as uncollectable
 * @param obj_ptr Pointer to the object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_unmark_uncollectable(void* obj_ptr);

/**
 * Check if an object is marked as uncollectable
 * @param obj_ptr Pointer to the object
 * @return 1 if uncollectable, 0 if not
 */
int32_t py_gc_is_uncollectable(void* obj_ptr);

// Debug and Utility Functions

/**
 * Debug untrack an object
 * @param obj_ptr Pointer to the object
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_debug_untrack(void* obj_ptr);

/**
 * Print debug state information
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_debug_state(void);

/**
 * Clear uncollectable objects
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_clear_uncollectable(void);

/**
 * Clear object registry
 * @return GC_SUCCESS on success, error code on failure
 */
gc_return_code_t py_gc_clear_registry(void);

#ifdef __cplusplus
}
#endif

#endif // PYTHON_GC_H 