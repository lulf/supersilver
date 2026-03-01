use vcad::{centered_cube, centered_cylinder, Part};

const THICKNESS: f64 = 2.0;
const HOLERADIUS: f64 = 1.25;

fn main() {
    let rotary = centered_cube("rotary", 26.0, 26.0, THICKNESS);
    let rotary = rotary.translate(0.0, 25.0, 0.0);

    let rotary_hole =
        centered_cylinder("rotary hole", HOLERADIUS, THICKNESS, 32).translate(0.0, 25.0, 0.0);
    let rotary_holes = rotary_hole
        .linear_pattern(0.0, 21.213204, 0.0, 2)
        .linear_pattern(21.213204, 0.0, 0.0, 2)
        .translate(-10.606602, -10.606602, 0.0);

    let rotary_cube = centered_cube("rotary cube", 12.0, 12.0, THICKNESS)
        .rotate(0.0, 0.0, 45.0)
        .translate(0.0, 25.0, 0.0);

    let part = rotary - rotary_holes - rotary_cube; // operator overloads for CSG
    part.write_stl("knobplate.stl").unwrap();

    let front = centered_cylinder("front plate", 9.0, THICKNESS, 32);
    let front_hole = centered_cylinder("front hole", 3.5, THICKNESS, 32);

    let plate = front - front_hole;
    plate.write_stl("plate.stl").unwrap();
}
