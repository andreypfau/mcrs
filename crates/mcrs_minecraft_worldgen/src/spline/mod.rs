use crate::spline::CubicSpline::Constant;
use bevy_math::FloatExt;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

pub trait SplineFunction<C>: RangeFunction {
    fn apply(&self, ctx: &C) -> f32;
}

pub trait RangeFunction {
    fn min_value(&self) -> f32;

    fn max_value(&self) -> f32;
}

#[derive(PartialEq)]
pub enum CubicSpline<C, F: SplineFunction<C>> {
    Constant(f32),
    MultiPoint {
        coordinate: F,
        locations: Vec<f32>,
        values: Vec<CubicSpline<C, F>>,
        derivatives: Vec<f32>,
        min_value: f32,
        max_value: f32,
        _phantom_data: PhantomData<C>,
    },
}

impl<C, F: SplineFunction<C> + Debug> Debug for CubicSpline<C, F> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Constant(v) => write!(f, "{}", v),
            CubicSpline::MultiPoint {
                coordinate,
                locations,
                values,
                derivatives,
                min_value,
                max_value,
                ..
            } => f
                .debug_struct("MultiPoint")
                .field("min_value", min_value)
                .field("max_value", max_value)
                .field("locations", locations)
                .field("derivatives", derivatives)
                .field("coordinate", coordinate)
                .field("values", values)
                .finish(),
        }
    }
}

impl<C, F: SplineFunction<C> + Clone> Clone for CubicSpline<C, F> {
    fn clone(&self) -> Self {
        match self {
            Constant(v) => Constant(*v),
            CubicSpline::MultiPoint {
                coordinate,
                locations,
                values,
                derivatives,
                min_value,
                max_value,
                _phantom_data,
            } => CubicSpline::MultiPoint {
                coordinate: coordinate.clone(),
                locations: locations.clone(),
                values: values.clone(),
                derivatives: derivatives.clone(),
                min_value: *min_value,
                max_value: *max_value,
                _phantom_data: PhantomData,
            },
        }
    }
}

impl<C, F: SplineFunction<C>> CubicSpline<C, F> {
    pub fn multipoint(
        coordinate: F,
        locations: Vec<f32>,
        values: Vec<CubicSpline<C, F>>,
        derivatives: Vec<f32>,
    ) -> Self {
        let n = locations.len() - 1;
        let mut spline_min = f32::INFINITY;
        let mut spline_max = f32::NEG_INFINITY;

        let coordinate_min = coordinate.min_value();
        let coordinate_max = coordinate.max_value();
        if coordinate_min < locations[0] {
            let extend_min = linear_extend(
                coordinate_min,
                &locations,
                values[0].min_value(),
                &derivatives,
                0,
            );
            let extend_max = linear_extend(
                coordinate_min,
                &locations,
                values[0].max_value(),
                &derivatives,
                0,
            );
            spline_min = spline_min.min(extend_min.min(extend_max));
            spline_max = spline_max.max(extend_min.max(extend_max));
        }
        if coordinate_max > locations[n] {
            let extend_min = linear_extend(
                coordinate_max,
                &locations,
                values[n].min_value(),
                &derivatives,
                n,
            );
            let extend_max = linear_extend(
                coordinate_max,
                &locations,
                values[n].max_value(),
                &derivatives,
                n,
            );
            spline_min = spline_min.min(extend_min.min(extend_max));
            spline_max = spline_max.max(extend_min.max(extend_max));
        }
        values.iter().for_each(|v| {
            spline_min = spline_min.min(v.min_value());
            spline_max = spline_max.max(v.max_value());
        });
        for i in 0..n {
            let location_left = locations[i];
            let location_right = locations[i + 1];
            let location_delta = location_right - location_left;
            let min_left = values[i].min_value();
            let max_left = values[i].max_value();
            let min_right = values[i + 1].min_value();
            let max_right = values[i + 1].max_value();
            let derivative_left = derivatives[i];
            let derivative_right = derivatives[i + 1];
            if derivative_left != 0.0 || derivative_right != 0.0 {
                let max_value_delta_left = derivative_left * location_delta;
                let max_value_delta_right = derivative_right * location_delta;
                let min_value = min_left.min(min_right);
                let max_value = max_left.max(max_right);
                let min_delta_left = max_value_delta_left - max_right + min_left;
                let max_delta_left = max_value_delta_left - min_right + max_left;
                let min_delta_right = -max_value_delta_right + min_right - min_left;
                let max_delta_right = -max_value_delta_right + max_right - min_left;
                let min_delta = min_delta_left.min(min_delta_right);
                let max_delta = max_delta_left.max(max_delta_right);
                spline_min = spline_min.min(min_value + 0.25 * min_delta);
                spline_max = spline_max.max(max_value + 0.25 * max_delta);
            }
        }
        CubicSpline::MultiPoint {
            coordinate,
            locations,
            values,
            derivatives,
            min_value: spline_min,
            max_value: spline_max,
            _phantom_data: PhantomData,
        }
    }

    pub fn map_all<V>(self, visitor: &mut V) -> Self
    where
        V: FnMut(F) -> F,
    {
        match self {
            Constant(_) => self,
            CubicSpline::MultiPoint {
                coordinate,
                locations,
                values,
                derivatives,
                ..
            } => {
                let coordinate = visitor(coordinate);
                let values = values.into_iter().map(|v| v.map_all(visitor)).collect();
                Self::multipoint(coordinate, locations, values, derivatives)
            }
        }
    }
}

impl<C, F: SplineFunction<C>> SplineFunction<C> for CubicSpline<C, F> {
    fn apply(&self, ctx: &C) -> f32 {
        match self {
            Constant(v) => *v,
            CubicSpline::MultiPoint {
                coordinate,
                locations,
                values,
                derivatives,
                ..
            } => {
                let coordinate = coordinate.apply(ctx);
                let i = find_interval_start(locations, coordinate);
                let n = locations.len() as isize - 1;
                if i < 0 {
                    linear_extend(coordinate, locations, values[0].apply(ctx), derivatives, 0)
                } else if i >= n {
                    linear_extend(
                        coordinate,
                        locations,
                        values[n as usize].apply(ctx),
                        derivatives,
                        n as usize,
                    )
                } else {
                    let i = i as usize;
                    let loc0 = locations[i];
                    let loc1 = locations[i + 1];
                    let der0 = derivatives[i];
                    let der1 = derivatives[i + 1];
                    let f = (coordinate - loc0) / (loc1 - loc0);

                    let value0 = values[i].apply(ctx);
                    let value1 = values[i + 1].apply(ctx);

                    let f8 = der0 * (loc1 - loc0) - (value1 - value0);
                    let f9 = -der1 * (loc1 - loc0) + (value1 - value0);

                    value0.lerp(value1, f) + f * (1.0 - f) * f8.lerp(f9, f)
                }
            }
        }
    }
}

impl<C, F: SplineFunction<C>> RangeFunction for CubicSpline<C, F> {
    fn min_value(&self) -> f32 {
        match self {
            Constant(v) => *v,
            CubicSpline::MultiPoint { min_value, .. } => *min_value,
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            Constant(v) => *v,
            CubicSpline::MultiPoint { max_value, .. } => *max_value,
        }
    }
}

impl<C, F: SplineFunction<C>> From<f32> for CubicSpline<C, F> {
    #[inline]
    fn from(value: f32) -> Self {
        Constant(value)
    }
}

pub struct Builder<C, F: SplineFunction<C>> {
    coordinate: F,
    locations: Vec<f32>,
    values: Vec<CubicSpline<C, F>>,
    derivatives: Vec<f32>,
}

impl<C, F: SplineFunction<C>> Builder<C, F> {
    pub fn new(coordinate: F) -> Self {
        Self {
            coordinate,
            locations: Vec::new(),
            values: Vec::new(),
            derivatives: Vec::new(),
        }
    }

    pub fn add_point<V: Into<CubicSpline<C, F>>>(
        mut self,
        location: f32,
        value: V,
        derivative: f32,
    ) -> Self {
        self.locations.push(location);
        self.values.push(value.into());
        self.derivatives.push(derivative);
        self
    }

    pub fn build(self) -> CubicSpline<C, F> {
        CubicSpline::multipoint(
            self.coordinate,
            self.locations,
            self.values,
            self.derivatives,
        )
    }
}

impl<C, F: SplineFunction<C>> From<Builder<C, F>> for CubicSpline<C, F> {
    #[inline]
    fn from(value: Builder<C, F>) -> Self {
        value.build()
    }
}

#[inline]
fn find_interval_start(locations: &[f32], point: f32) -> isize {
    binary_search(0, locations.len(), |i| point < locations[i]) as isize - 1
}

fn linear_extend(point: f32, locations: &[f32], value: f32, derivatives: &[f32], i: usize) -> f32 {
    let f = derivatives[i];
    if f == 0.0 {
        value
    } else {
        value + f * (point - locations[i])
    }
}

fn binary_search<F>(min: usize, max: usize, predicate: F) -> usize
where
    F: Fn(usize) -> bool,
{
    let mut min = min;
    let mut max = max;
    while min < max {
        let mid = min + (max - min) / 2;
        if predicate(mid) {
            max = mid;
        } else {
            min = mid + 1;
        }
    }
    min
}

#[cfg(test)]
mod test {
    use crate::spline::test::CoordinateFunction::Identity;
    use crate::spline::{Builder, RangeFunction, SplineFunction};
    use CoordinateFunction::Square;

    enum CoordinateFunction {
        Identity,
        Square,
    }

    impl RangeFunction for CoordinateFunction {
        fn min_value(&self) -> f32 {
            f32::NEG_INFINITY
        }

        fn max_value(&self) -> f32 {
            f32::INFINITY
        }
    }

    impl SplineFunction<f32> for CoordinateFunction {
        fn apply(&self, ctx: &f32) -> f32 {
            match self {
                Identity => *ctx,
                Square => *ctx * *ctx,
            }
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn simple() {
        let spline = Builder::new(Identity)
            .add_point(-1.1, 0.044, 0.0)
            .add_point(-1.02, -0.2222, 0.0)
            .add_point(-0.51, -0.2222, 0.0)
            .add_point(-0.44, -0.12, 0.0)
            .add_point(-0.18, -0.12, 0.0)
            .build();
        assert_eq!(spline.apply(&mut -1.6), 0.044);
        assert_eq!(spline.apply(&mut -0.7), -0.2222);
        assert_eq!(spline.apply(&mut -0.2), -0.12);
        assert!(close(spline.apply(&mut -0.5), -0.21653879));
    }

    #[test]
    fn derivatives() {
        let spline = Builder::new(Identity)
            .add_point(0.0, 0.0178, 0.2)
            .add_point(0.3, 0.23, 0.7)
            .add_point(0.46, 0.89, -0.03)
            .add_point(0.6, 0.4, 0.0)
            .build();
        assert_eq!(spline.apply(&mut 0.0), 0.0178);
        assert!(close(spline.apply(&mut -0.1), -0.0022000019));
        assert!(close(spline.apply(&mut 0.31), 0.24358201));
        assert!(close(spline.apply(&mut 0.4), 0.69171876));
    }

    #[test]
    fn nested() {
        let mut spline = Builder::new(Identity)
            .add_point(0.0, 0.23, 0.0)
            .add_point(
                0.2,
                Builder::new(Square)
                    .add_point(-0.1, 0.0, 0.0)
                    .add_point(1.2, 0.4, 0.0),
                0.0,
            )
            .add_point(0.7, 0.7, 0.0)
            .build();
        assert!(close(spline.apply(&mut 0.3), 0.09352946));
    }
}
