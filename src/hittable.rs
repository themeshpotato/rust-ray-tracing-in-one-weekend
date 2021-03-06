use crate::math::*;
use crate::ray::*;
use crate::material::*;
use crate::aabb::*;

#[derive(Default)]
pub struct HitRecord {
    pub point: Point3,
    pub normal: Vector3,
    pub t: f64,
    pub front_face: bool,
    pub mat_handle: MaterialHandle,
    pub u: f64,
    pub v: f64
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

#[derive(Clone)]
pub enum Hittable {
    Sphere          { mat_handle: MaterialHandle, center: Point3, radius: f64 },
    MovingSphere    { mat_handle: MaterialHandle, center_0: Point3, center_1: Point3, time_0: f64, time_1: f64, radius: f64 },
    BvhNode         { left: Box<Hittable>, right: Box<Hittable>, aabb_box: AABB },
    XYRect          { mat_handle: MaterialHandle, x0: f64, x1: f64, y0: f64, y1: f64, k: f64 },
    XZRect          { mat_handle: MaterialHandle, x0: f64, x1: f64, z0: f64, z1: f64, k: f64 },
    YZRect          { mat_handle: MaterialHandle, y0: f64, y1: f64, z0: f64, z1: f64, k: f64 },
    Box             { mat_handle: MaterialHandle, min: Point3, max: Point3, sides: Vec<Hittable> },
    Translate       { offset: Vector3, ptr: Box<Hittable> },
    RotateY         { sin_theta: f64, cos_theta: f64, has_box: bool, bbox: AABB, ptr: Box<Hittable> },
    ConstantMedium  { phase_function: MaterialHandle, boundary: Box<Hittable>, neg_inv_density: f64 }
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

pub fn hittables_bounding_box(hittables: &Vec<Hittable>, time_0: f64, time_1: f64) -> Option<AABB> {
    if hittables.len() == 0 {
        return None;
    }

    let mut final_box = None;

    for h in hittables {
        match h.bounding_box(time_0, time_1) {
            None => { return None; },
            Some(b) => {
                final_box = Some(b);
            }
        }
    }

    final_box
}

impl Hittable {
    pub fn new_bvh_node(list: &Vec<Hittable>, start: usize, end: usize, time_0: f64, time_1: f64) -> Hittable {
        let mut cpy = list.clone();
        let left;
        let right;

        let axis = random_int_range(0, 2);
        let comparator = match axis { 
            0 => {
                AABB::box_x_compare
            },
            1 => {
                AABB::box_y_compare
            },
            _ => {
                AABB::box_z_compare
            }
        };

        let object_span = end - start;
        if object_span == 1 { 
            left = Box::new(cpy[start].clone());
            right = Box::new(cpy[start].clone()); 
        } else if object_span == 2 {
            if comparator(&cpy[start], &cpy[start + 1]) == std::cmp::Ordering::Less {
                left = Box::new(cpy[start].clone());
                right = Box::new(cpy[start + 1].clone());
            } else {
                left = Box::new(cpy[start + 1].clone());
                right = Box::new(cpy[start].clone());
            }
        } else {
            unsafe {
                cpy[start..end].sort_by(comparator);
            }
            let mid = start + object_span / 2;
            left = Box::new(Self::new_bvh_node(&cpy, start, mid, time_0, time_1));
            right = Box::new(Self::new_bvh_node(&cpy, mid, end, time_0, time_1));
        }

        let aabb_box = {
            if let (Some(box_left), Some(box_right)) = (left.bounding_box(time_0, time_1), right.bounding_box(time_0, time_1)) {
                AABB::surrounding_box(&box_left, &box_right)
            } else {
                eprintln!("No bounding box in BVHNode");
                AABB::new(Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 0.0, 0.0))
            }
        };
        
        Hittable::BvhNode {
            left,
            right,
            aabb_box
        }
    }

    pub fn new_box(min: Point3, max: Point3, mat_handle: MaterialHandle) -> Hittable {
        let mut sides = Vec::new();

        sides.push(Hittable::XYRect { mat_handle, x0: min.x, x1: max.x, y0: min.y, y1: max.y, k: max.z });
        sides.push(Hittable::XYRect { mat_handle, x0: min.x, x1: max.x, y0: min.y, y1: max.y, k: min.z });

        sides.push(Hittable::XZRect { mat_handle, x0: min.x, x1: max.x, z0: min.z, z1: max.z, k: max.y });
        sides.push(Hittable::XZRect { mat_handle, x0: min.x, x1: max.x, z0: min.z, z1: max.z, k: min.y });

        sides.push(Hittable::YZRect { mat_handle, y0: min.y, y1: max.y, z0: min.z, z1: max.z, k: max.x });
        sides.push(Hittable::YZRect { mat_handle, y0: min.y, y1: max.y, z0: min.z, z1: max.z, k: min.x });

        Hittable::Box { mat_handle, min, max, sides }
    }

    pub fn new_rotate_y(angle: f64, hittable: Hittable) -> Hittable {
        let radians = degrees_to_radians(angle);
        let sin_theta = f64::sin(radians);
        let cos_theta = f64::cos(radians);

        let has_box;
        let aabb;
        
        if let Some(bbox) = hittable.bounding_box(0.0, 1.0) {
            has_box = true;
            aabb = bbox;
        } else {
            has_box = false;
            aabb = AABB::new(Point3::new(0.0, 0.0, 0.0,), Point3::new(0.0, 0.0, 0.0));
        }

        let mut min = [f64::INFINITY; 3];
        let mut max = [-f64::INFINITY; 3];

        for i in 0..2 {
            for j in 0..2 {
                for k in 0..2 {
                    let i = i as f64;
                    let j = j as f64;
                    let k = k as f64;

                    let x = i * aabb.maximum.x + (1.0 - i) * aabb.minimum.x;
                    let y = j * aabb.maximum.y + (1.0 - j) * aabb.minimum.y;
                    let z = k * aabb.maximum.z + (1.0 - k) * aabb.minimum.z;

                    let newx = cos_theta * x + sin_theta * z;
                    let newz = -sin_theta * x + cos_theta * z;

                    let tester = [newx, y, newz];

                    for c in 0..3 {
                        min[c] = f64::min(min[c], tester[c]);
                        max[c] = f64::max(max[c], tester[c]);
                    }
                }
            }
        }

        let aabb = AABB::new(Point3::new(min[0], min[1], min[2]), Point3::new(max[0], max[1], max[2]));

        Hittable::RotateY {
            sin_theta,
            cos_theta,
            has_box,
            bbox: aabb,
            ptr: Box::new(hittable)
        }
    }

    pub fn new_constant_medium(hittable: Hittable, d: f64, mat_handle: MaterialHandle) -> Hittable {
        Hittable::ConstantMedium {
            phase_function: mat_handle,
            boundary: Box::new(hittable),
            neg_inv_density: -1.0 / d
        }
    }

    pub fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        match self {
            Hittable::Sphere { mat_handle, center, radius } => {
                Self::sphere_hit(&center, *radius, ray, t_min, t_max, *mat_handle)
            },
            Hittable::MovingSphere { mat_handle, center_0, center_1, time_0, time_1, radius } => {
                Self::sphere_hit(&Self::get_center_at_time(center_0, center_1, *time_0, *time_1, ray.time), *radius, ray, t_min, t_max, *mat_handle)
            },
            Hittable::BvhNode { left, right, aabb_box } => {
                Self::bvh_node_hit(left, right, aabb_box, ray, t_min, t_max)
            },
            Hittable::XYRect { mat_handle, x0, x1, y0, y1, k } => {
                Self::xy_rect_hit(*x0, *x1, *y0, *y1, *k, ray, t_min, t_max, *mat_handle)
            },
            Hittable::XZRect { mat_handle, x0, x1, z0, z1, k } => {
                Self::xz_rect_hit(*x0, *x1, *z0, *z1, *k, ray, t_min, t_max, *mat_handle)
            },
            Hittable::YZRect { mat_handle, y0, y1, z0, z1, k } => {
                Self::yz_rect_hit(*y0, *y1, *z0, *z1, *k, ray, t_min, t_max, *mat_handle)
            },
            Hittable::Box { mat_handle, min, max, sides } => {
                hit_hittables(sides, ray, t_min, t_max)
            },
            Hittable::Translate { offset, ptr } => {
                let moved_ray = Ray::with_time(ray.origin - *offset, ray.direction, ray.time);

                if let Some(mut rec) = ptr.hit(&moved_ray, t_min, t_max) {
                    rec.point += *offset;
                    let normal = rec.normal;
                    rec.set_face_normal(&moved_ray, &normal);

                    Some(rec)
                } else {
                    None
                }
            },
            Hittable::RotateY { sin_theta, cos_theta, has_box: _, bbox: _, ptr } => {
                Self::hit_rotate_y(*sin_theta, *cos_theta, ptr, ray, t_min, t_max)
            },
            Hittable::ConstantMedium { phase_function, boundary, neg_inv_density } => {
                Self::hit_constant_medium(boundary, *phase_function, *neg_inv_density, ray, t_min, t_max)
            }
        }
    }

    fn sphere_hit(center: &Point3, radius: f64, ray: &Ray, t_min: f64, t_max: f64, mat_handle: MaterialHandle) -> Option<HitRecord> {
        let oc = ray.origin - *center;
        let a = ray.direction.length_squared();
        let half_b = Vector3::dot(&oc, &ray.direction);
        let c = oc.length_squared() - radius * radius;
        let discriminant = half_b * half_b - a * c;
        if discriminant < 0.0 {
             return None;
        } 

        let sqrtd = f64::sqrt(discriminant);
        
        // Find the nearest root that lies in the acceptable range
        let mut root = (-half_b - sqrtd) / a;
        if root < t_min || t_max < root {
            root = (-half_b + sqrtd) / a;
            if root < t_min || t_max < root {
                return None;
            }
        }
        
        let mut rec = HitRecord::new();

        rec.mat_handle = mat_handle;
        rec.t = root;
        rec.point = ray.at(rec.t);
        let outward_normal = (rec.point - *center) / radius;
        rec.set_face_normal(ray, &outward_normal);

        let (u, v) = sphere_uv(&outward_normal);
        rec.u = u;
        rec.v = v;

        Some(rec)
    }

    fn bvh_node_hit(left: &Box<Hittable>, right: &Box<Hittable>, aabb: &AABB, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        if !aabb.hit(ray, t_min, t_max) {
            return None;
        }

        if let Some(hit_left) = left.hit(ray, t_min, t_max) {
            if let Some(hit_right) = right.hit(ray, t_min, hit_left.t) {
                Some(hit_right)
            } else {
                Some(hit_left)
            }
        } else if let Some(hit_right) = right.hit(ray, t_min, t_max) {
            Some(hit_right)
        } else {
            None
        }
    }

    fn xy_rect_hit(x0: f64, x1: f64, y0: f64, y1: f64, k: f64, ray: &Ray, t_min: f64, t_max: f64, mat_handle: MaterialHandle) -> Option<HitRecord> {
        let t = (k - ray.origin.z) / ray.direction.z;
        
        if t < t_min || t > t_max {
            return None;
        }

        let x = ray.origin.x + t * ray.direction.x;
        let y = ray.origin.y + t * ray.direction.y;

        if x < x0 || x > x1 || y < y0 || y > y1 {
            return None;
        }

        let mut rec = HitRecord::new();
        rec.u = (x - x0) / (x1 - x0);
        rec.v = (y - y0) / (y1 - y0);
        rec.t = t;
        let outward_normal = Vector3::new(0.0, 0.0, 1.0);
        rec.set_face_normal(ray, &outward_normal);
        rec.mat_handle = mat_handle;
        rec.point = ray.at(t);

        Some(rec)
    }

    fn xz_rect_hit(x0: f64, x1: f64, z0: f64, z1: f64, k: f64, ray: &Ray, t_min: f64, t_max: f64, mat_handle: MaterialHandle) -> Option<HitRecord> {
        let t = (k - ray.origin.y) / ray.direction.y;

        if t < t_min || t > t_max {
            return None
        }

        let x = ray.origin.x + t * ray.direction.x;
        let z = ray.origin.z + t * ray.direction.z;

        if x < x0 || x > x1 || z < z0 || z > z1 {
            return None;
        }

        let mut rec = HitRecord::new();
        rec.u = (x - x0) / (x1 - x0);
        rec.v = (z - z0) / (z1 - z0);
        rec.t = t;
        let outward_normal = Vector3::new(0.0, 1.0, 0.0);
        rec.set_face_normal(ray, &outward_normal);
        rec.mat_handle = mat_handle;
        rec.point = ray.at(t);

        Some(rec)
    }

    fn yz_rect_hit(y0: f64, y1: f64, z0: f64, z1: f64, k: f64, ray: &Ray, t_min: f64, t_max: f64, mat_handle: MaterialHandle) -> Option<HitRecord> {
        let t = (k - ray.origin.x) / ray.direction.x;

        if t < t_min || t > t_max {
            return None
        }

        let y = ray.origin.y + t * ray.direction.y;
        let z = ray.origin.z + t * ray.direction.z;

        if y < y0 || y > y1 || z < z0 || z > z1 {
            return None;
        }

        let mut rec = HitRecord::new();
        rec.u = (y - y0) / (y1 - y0);
        rec.v = (z - z0) / (z1 - z0);
        rec.t = t;
        let outward_normal = Vector3::new(1.0, 0.0, 0.0);
        rec.set_face_normal(ray, &outward_normal);
        rec.mat_handle = mat_handle;
        rec.point = ray.at(t);

        Some(rec)
    }

    fn hit_rotate_y(sin_theta: f64, cos_theta: f64, ptr: &Box<Hittable>, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let mut origin = ray.origin;
        let mut direction = ray.direction;

        origin.x = cos_theta * ray.origin.x - sin_theta * ray.origin.z;
        origin.z = sin_theta * ray.origin.x + cos_theta * ray.origin.z;

        direction.x = cos_theta * ray.direction.x - sin_theta * ray.direction.z;
        direction.z = sin_theta * ray.direction.x + cos_theta * ray.direction.z;

        let rotated_ray = Ray::with_time(origin, direction, ray.time);

        if let Some(mut rec) = ptr.hit(&rotated_ray, t_min, t_max) {
            let mut p = rec.point;
            let mut normal = rec.normal;

            p.x = cos_theta * rec.point.x + sin_theta * rec.point.z;
            p.z = -sin_theta * rec.point.x + cos_theta * rec.point.z;

            normal.x = cos_theta * rec.normal.x + sin_theta * rec.normal.z;
            normal.z = -sin_theta * rec.normal.x + cos_theta * rec.normal.z;

            rec.point = p;
            rec.set_face_normal(&rotated_ray, &normal);

            Some(rec)
        } else {
            None
        }
    }

    fn hit_constant_medium(boundary: &Box<Hittable>, phase_function: MaterialHandle, neg_inv_density: f64, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        // Print occasional samples when debugging. To enable, set enable_debug true.
        const ENABLE_DEBUG: bool = false;
        let debugging : bool = ENABLE_DEBUG && random_double() < 0.00001;

        if let Some(mut rec1) = boundary.hit(ray, -f64::INFINITY, f64::INFINITY) {
            if let Some(mut rec2) = boundary.hit(ray, rec1.t + 0.0001, f64::INFINITY) {
                if debugging {
                    eprintln!("t_min={}, t_max={}", rec1.t, rec2.t);
                }

                if rec1.t < t_min {
                    rec1.t = t_min;
                }

                if rec2.t > t_max {
                    rec2.t = t_max;
                }

                if rec1.t >= rec2.t {
                    return None;
                }

                if rec1.t < 0.0 {
                    rec1.t = 0.0;
                }

                let ray_length = ray.direction.length();
                let distance_inside_boundary = (rec2.t - rec1.t) * ray_length;
                let hit_distance = neg_inv_density * f64::ln(random_double());

                if hit_distance > distance_inside_boundary {
                    return None;
                }
                
                let mut rec = HitRecord::new();
                rec.t = rec1.t + hit_distance / ray_length;
                rec.point = ray.at(rec.t);

                if debugging {
                    eprintln!("hit_distance = {}\nrec.t = {}\nrec.point = {}", hit_distance, rec.t, rec.point);
                }

                rec.normal = Vector3::new(1.0, 0.0, 0.0);
                rec.front_face = true;
                rec.mat_handle = phase_function;

                Some(rec)
            } else {
                return None;
            }
        } else {
            return None;
        }


    }

    pub fn bounding_box(&self, time_0: f64, time_1: f64) -> Option<AABB> {
        match self {
            Hittable::Sphere { mat_handle: _, center, radius } => {
                Self::sphere_bounding_box(&center, *radius)
            },
            Hittable::MovingSphere { mat_handle: _, center_0, center_1, time_0, time_1, radius } => {
                Self::moving_sphere_bounding_box(&center_0, &center_1, *radius, *time_0, *time_1)
            },
            Hittable::BvhNode { left: _, right: _, aabb_box } => {
                Some(*aabb_box)
            },
            Hittable::XYRect { mat_handle, x0, x1, y0, y1, k } => {
                Some(AABB::new(
                    Point3::new(*x0, *y0, *k - 0.0001),
                    Point3::new(*x1, *y1, *k + 0.0001)
                ))
            },
            Hittable::XZRect { mat_handle, x0, x1, z0, z1, k } => {
                Some(AABB::new(
                    Point3::new(*x0, *k - 0.0001, *z0),
                    Point3::new(*x1, *k + 0.0001, *z1)
                ))
            },
            Hittable::YZRect { mat_handle, y0, y1, z0, z1, k } => {
                Some(AABB::new(
                    Point3::new(*k - 0.0001, *y0, *z0),
                    Point3::new(*k + 0.0001, *y1, *z1)
                ))
            },
            Hittable::Box { mat_handle, min, max, sides } => {
                Some(AABB::new(*min, *max))
            },
            Hittable::Translate { offset, ptr } => {
                if let Some(aabb) = ptr.bounding_box(time_0, time_1) {
                    Some(AABB::new(
                        aabb.minimum + *offset,
                        aabb.maximum + *offset
                    ))
                } else {
                    None
                }
            },
            Hittable::RotateY { sin_theta: _, cos_theta: _, has_box, bbox, ptr: _ } => {
                if *has_box {
                    Some(*bbox)
                } else {
                    None
                }
            },
            Hittable::ConstantMedium { phase_function: _, boundary, neg_inv_density: _ } => {
                boundary.bounding_box(time_0, time_1)
            }
        }
    }

    fn sphere_bounding_box(center: &Point3, radius: f64) -> Option<AABB> {
        Some(
            AABB::new(
                *center - Vector3::new(radius, radius, radius),
                *center + Vector3::new(radius, radius, radius)
            )
        )
    }

    fn moving_sphere_bounding_box(center_0: &Point3, center_1: &Point3, radius: f64, time_0: f64, time_1: f64) -> Option<AABB> {
        let c0 = Self::get_center_at_time(center_0, center_1, time_0, time_1, time_0);
        let c1 = Self::get_center_at_time(center_0, center_1, time_0, time_1, time_1);

        let box0 = AABB::new(
            c0 - Vector3::new(radius, radius, radius),
            c0 + Vector3::new(radius, radius, radius)
        );

        let box1 = AABB::new(
            c1 - Vector3::new(radius, radius, radius),
            c1 + Vector3::new(radius, radius, radius)
        );

        Some(AABB::surrounding_box(&box0, &box1))
    }

    fn get_center_at_time(center_0: &Point3, center_1: &Point3, time_0: f64, time_1: f64, time: f64) -> Point3 {
        *center_0 + ((time - time_0) / (time_1 - time_0)) * (*center_1 - *center_0)
    }
}
