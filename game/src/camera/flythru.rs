use {
    ultraviolet as uv,
};

pub struct FlythruCam {
    pub(crate) points: Vec<uv::Vec3>,
    t: f32,
}

impl FlythruCam {
    pub fn new(graph: &[bundle::ArchivedTrackNode]) -> Self {
        let mut points = Vec::new();
        let mut node_i = 0;

        let mut step = |node_i: u32| {
            let s = &graph[node_i as usize];
            points.push(s.center.into());
            s.next.map(|i| Some(i).filter(|&i| i != !0u32))
        };

        loop {
            let [n0, n1] = step(node_i);
            node_i = n0.or(n1).unwrap_or(0);
            if node_i == 0 {break}
        }

        loop {
            let [n0, n1] = step(node_i);
            node_i = n1.or(n0).unwrap_or(0);
            if node_i == 0 {break}
        }

        Self{points, t: 0.}
    }

    fn eval(&self, t: f32) -> uv::Vec3 {
        let t = t.fract();

        let u = self.points.len() as f32 * t;
        let i0 = u as i32;
        let t = u - i0 as f32;

        let t2 = t * t;
        let t3 = t2 * t;
        let ts = uv::Vec4::new(t3, t2, t, 1.);

        const BASIS: uv::Mat4 = uv::Mat4::new(
            uv::Vec4::new( 2.,  1., 1.,-2.),
            uv::Vec4::new(-3., -2.,-1., 3.),
            uv::Vec4::new( 0.,  1., 0., 0.),
            uv::Vec4::new( 1.,  0., 0., 0.),
        );

        let hs = BASIS * ts;
        let [h0,h1,h2,h3]: [f32; 4] = hs.into();

        /*
        let h0 = ts.dot([ 2., -3., 0., 1.].into());
        let h1 = ts.dot([ 1., -2., 1., 0.].into());
        let h2 = ts.dot([ 1., -1., 0., 0.].into());
        let h3 = ts.dot([-2.,  3., 0., 0.].into());*/

        let [p0, p1, p2, p3]: [uv::Vec3; 4] = [i0-1, i0, i0+1, i0+2]
            .map(|i| self.points[i.rem_euclid(self.points.len() as i32) as usize]);

        let m0 = 0.5 * (p2 - p0);
        let m1 = 0.5 * (p3 - p1);

        h0*p1 + h1*m0 + h2*m1 + h3*p2
    }

    pub fn update(&mut self) -> uv::Isometry3 {
        let t_pos   = self.t;
        let t_focus = t_pos + 0.004;
        let offset = uv::Vec3::unit_y() * -400.;

        let pos   = self.eval(t_pos)   + offset;
        let focus = self.eval(t_focus) + offset;

        let facing = (focus - pos).normalized();

        let pan = uv::Rotor3::from_rotation_between(
            uv::Vec3::unit_z(),
            (facing * uv::Vec3::new(1., 0., 1.)).normalized(),
        );

        let tilt = uv::Rotor3::from_rotation_between(
            uv::Vec3::unit_z(),
            (facing * uv::Vec3::new(0., 1., 0.) + uv::Vec3::unit_z()).normalized(),
        );

        let rotate = pan * tilt;

        self.t += 0.00015;
        uv::Isometry3::new(pos, rotate)
    }
}

