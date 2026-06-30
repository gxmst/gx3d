use super::PhysicsWorld;
use glam::Vec3;
use rapier3d::prelude::QueryFilter;

pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3, max_distance: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            max_distance,
        }
    }

    fn to_rapier(&self) -> rapier3d::prelude::Ray {
        let origin = rapier3d::math::Vector::new(self.origin.x, self.origin.y, self.origin.z);
        let dir = rapier3d::math::Vector::new(self.direction.x, self.direction.y, self.direction.z);
        rapier3d::prelude::Ray::new(origin, dir)
    }
}

pub struct RaycastHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub entity: Option<hecs::Entity>,
    pub collider_handle: rapier3d::prelude::ColliderHandle,
}

fn vector_to_glam(v: rapier3d::math::Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

impl PhysicsWorld {
    pub fn cast_ray(&self, ray: &Ray) -> Option<RaycastHit> {
        let rapier_ray = ray.to_rapier();
        let filter = QueryFilter::default();
        let query_pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );

        query_pipeline
            .cast_ray_and_get_normal(&rapier_ray, ray.max_distance, true)
            .map(|(handle, intersection)| {
                let point = rapier_ray.point_at(intersection.time_of_impact);
                let normal = intersection.normal;
                RaycastHit {
                    point: vector_to_glam(point),
                    normal: vector_to_glam(normal),
                    distance: intersection.time_of_impact,
                    entity: self.collider_entity_map.get(&handle).copied(),
                    collider_handle: handle,
                }
            })
    }

    pub fn cast_ray_all(&self, ray: &Ray) -> Vec<RaycastHit> {
        let rapier_ray = ray.to_rapier();
        let filter = QueryFilter::default();
        let query_pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );

        let mut results: Vec<RaycastHit> = query_pipeline
            .intersect_ray(rapier_ray, ray.max_distance, true)
            .map(|(handle, _collider, intersection)| {
                let point = rapier_ray.point_at(intersection.time_of_impact);
                let normal = intersection.normal;
                RaycastHit {
                    point: vector_to_glam(point),
                    normal: vector_to_glam(normal),
                    distance: intersection.time_of_impact,
                    entity: self.collider_entity_map.get(&handle).copied(),
                    collider_handle: handle,
                }
            })
            .collect();

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        results
    }
}
