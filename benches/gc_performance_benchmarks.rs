use criterion::{Criterion, black_box, criterion_group, criterion_main};
use python_gc::{GarbageCollector, PyObject, object::ObjectData};

fn create_test_objects(count: usize) -> Vec<PyObject> {
    static NAMES: [&str; 3] = ["list", "dict", "set"];

    (0..count)
        .map(|i| {
            let name_idx = i % 3;
            let name = NAMES[name_idx];

            match name_idx {
                0 => PyObject::new(name.to_string(), ObjectData::List(Vec::new())),
                1 => PyObject::new(name.to_string(), ObjectData::Dict(Vec::new())),
                _ => PyObject::new(name.to_string(), ObjectData::Set(Vec::new())),
            }
        })
        .collect()
}

fn benchmark_object_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Object Creation");

    group.bench_function("create_1000_objects", |b| {
        b.iter(|| {
            let objects = create_test_objects(1000);
            black_box(objects)
        });
    });

    group.bench_function("create_10000_objects", |b| {
        b.iter(|| {
            let objects = create_test_objects(10000);
            black_box(objects)
        });
    });

    group.finish();
}

fn benchmark_object_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Object Tracking");

    group.bench_function("track_1000_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(1000);

            for obj in objects {
                gc.track(obj).unwrap();
            }

            black_box(gc.get_count());
        });
    });

    group.bench_function("track_10000_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(10000);

            for obj in objects {
                gc.track(obj).unwrap();
            }

            black_box(gc.get_count());
        });
    });

    group.bench_function("track_10000_objects_bulk", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(10000);

            gc.track_bulk(objects).unwrap();

            black_box(gc.get_count());
        });
    });

    group.finish();
}

fn benchmark_garbage_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Garbage Collection");

    group.bench_function("collect_empty_gc", |b| {
        b.iter(|| {
            let gc = GarbageCollector::new();
            black_box(gc.collect().unwrap());
        });
    });

    group.bench_function("collect_with_1000_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(1000);

            for obj in objects {
                gc.track(obj).unwrap();
            }

            black_box(gc.collect().unwrap());
        });
    });

    group.bench_function("collect_with_10000_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(10000);

            for obj in objects {
                gc.track(obj).unwrap();
            }

            black_box(gc.collect().unwrap());
        });
    });

    group.bench_function("collect_with_10000_objects_fast", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(10000);

            gc.track_bulk(objects).unwrap();

            black_box(gc.collect().unwrap());
        });
    });

    group.finish();
}

fn benchmark_generation_management(c: &mut Criterion) {
    let mut group = c.benchmark_group("Generation Management");

    group.bench_function("promote_generations", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();

            for i in 0..1000 {
                let obj = PyObject::new("test".to_string(), ObjectData::Integer(i as i64));
                gc.track(obj).unwrap();

                if i % 100 == 0 {
                    gc.collect().unwrap();
                }
            }

            black_box(gc.get_stats());
        });
    });

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Usage");

    group.bench_function("memory_tracking_10000", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();
            let objects = create_test_objects(10000);

            let estimated_memory = objects.len() * std::mem::size_of::<PyObject>();

            for obj in objects {
                gc.track(obj).unwrap();
            }

            black_box(estimated_memory);
        });
    });

    group.finish();
}

fn benchmark_python_object_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Python Object Tracking (Real Use Case)");

    group.bench_function("track_10000_python_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();

            for i in 0..10000 {
                let obj = PyObject::new_ffi(
                    "python_obj",
                    ObjectData::Integer(i as i64),
                    std::ptr::null_mut(),
                );
                gc.track(obj).unwrap();
            }

            black_box(gc.get_count());
        });
    });

    group.bench_function("collect_10000_python_objects", |b| {
        b.iter(|| {
            let mut gc = GarbageCollector::new();

            for i in 0..10000 {
                let obj = PyObject::new_ffi(
                    "python_obj",
                    ObjectData::Integer(i as i64),
                    std::ptr::null_mut(),
                );
                gc.track(obj).unwrap();
            }

            black_box(gc.collect().unwrap());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_object_creation,
    benchmark_object_tracking,
    benchmark_garbage_collection,
    benchmark_generation_management,
    benchmark_memory_usage,
    benchmark_python_object_tracking
);

criterion_main!(benches);
