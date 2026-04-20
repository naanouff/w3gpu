use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::{Quat, Vec3};
use w3gpu_ecs::{
    components::{HierarchyComponent, TransformComponent},
    World,
};

fn bench_transform_flat(c: &mut Criterion) {
    let counts = [1_000u32, 10_000, 100_000];

    for &n in &counts {
        let mut world = World::new();
        for i in 0..n {
            let e = world.create_entity();
            let mut t = TransformComponent::new(
                Vec3::new(i as f32 * 0.1, 0.0, 0.0),
                Quat::IDENTITY,
                Vec3::ONE,
            );
            t.dirty = true;
            world.add_component(e, t);
        }

        let label = format!("transform_flat_{n}");
        c.bench_function(&label, |b| {
            b.iter(|| {
                world.for_each_without_mut::<TransformComponent, HierarchyComponent, _>(|t| {
                    if t.dirty {
                        t.world_matrix = black_box(t.local_matrix);
                        t.dirty = false;
                    }
                    t.dirty = true; // reset for next iteration
                });
            })
        });
    }
}

criterion_group!(benches, bench_transform_flat);
criterion_main!(benches);
