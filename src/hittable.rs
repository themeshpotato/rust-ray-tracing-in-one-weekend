use crate::math::*;
use crate::ray::*;

#[derive(Default)]
pub struct HitRecord {
    pub point: Point3,
    pub normal: Vector3,
    pub t: f64,
    pub front_face: bool
}

impl HitRecord {
    pub fn new() -> HitRecord {
        HitRecord {
            ..Default::default()
        }
    }
    pub fn set_face_normal(&mut self, ray: &Ray, outward_normal: &Vector3) {
        self.front_face = Vector3::dot(&ray.direction, &outward_normal) < 0.0;
        self.normal = if self.front_face { *outward_normal } else { -outward_normal };
    }
}

pub enum Hittable {
    Sphere(Point3, f64)
}

pub fn hit_hittables(hittables: &Vec<Hittable>, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
    let mut closest_so_far = t_max;
    let mut rec: Option<HitRecord> = None;

    for hittable in hittables {
        if let Some(record) = hittable.hit(ray, t_min, closest_so_far) {
            closest_so_far = record.t;
            rec = Some(record)
        }
    }
    
    rec
}

impl Hittable {
    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        match self {
            Hittable::Sphere(center, radius) => {
                Self::sphere_hit(&center, *radius, ray, t_min, t_max)
            }
        }
    }

    fn sphere_hit(center: &Point3, radius: f64, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let oc = ray.origin - *center;
        let a = ray.direction.length_squared();
        let half_b = Vector3::dot(&oc, &ray.direction);
        let c = oc.length_squared() - radius * radius;
        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
             return None;
        } 

        let sqrtd = discriminant.sqrt();
        
        // Find the nearest root that lies in the acceptable range
        let mut root = (-half_b - sqrtd) / a;
        if root < t_min || t_max < root {
            root = (-half_b + sqrtd) / a;
            if root < t_min || t_max < root {
                return None;
            }
        }
        
        let mut rec = HitRecord::new();

        rec.t = root;
        rec.point = ray.at(rec.t);
        let outward_normal = (rec.point - *center) / radius;
        rec.set_face_normal(ray, &outward_normal);

        Some(rec)
    }
}