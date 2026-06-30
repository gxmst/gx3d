use gxengine::core::{EngineWorld, Resources, Schedule, Stage};

#[derive(Debug, Default)]
struct Counter(i32);

#[test]
fn stages_run_in_order() {
    let mut schedule = Schedule::new();
    let resources = Resources::new();
    resources.insert(Counter(0));

    schedule.add_system(Stage::PreUpdate, |_world, res| {
        res.get_mut::<Counter>().unwrap().0 += 1;
    });
    schedule.add_system(Stage::Update, |_world, res| {
        res.get_mut::<Counter>().unwrap().0 *= 10;
    });
    schedule.add_system(Stage::LateUpdate, |_world, res| {
        res.get_mut::<Counter>().unwrap().0 += 2;
    });

    let mut world = EngineWorld::new();

    schedule.run_stage(Stage::PreUpdate, &mut world, &resources);
    schedule.run_stage(Stage::Update, &mut world, &resources);
    schedule.run_stage(Stage::LateUpdate, &mut world, &resources);

    assert_eq!(resources.get::<Counter>().unwrap().0, 12);
}

#[test]
fn simultaneous_immutable_borrows_are_allowed() {
    let resources = Resources::new();
    resources.insert(Counter(0));
    resources.insert(String::from("hello"));

    let c = resources.get::<Counter>().unwrap();
    let s = resources.get::<String>().unwrap();
    assert_eq!(c.0, 0);
    assert_eq!(&*s, "hello");
}

#[test]
fn missing_resource_returns_none() {
    let resources = Resources::new();
    assert!(resources.get::<Counter>().is_none());
}
