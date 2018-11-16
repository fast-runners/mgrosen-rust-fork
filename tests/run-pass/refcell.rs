use std::cell::RefCell;

fn lots_of_funny_borrows() {
    let c = RefCell::new(42);
    {
        let s1 = c.borrow();
        let _x: i32 = *s1;
        let s2 = c.borrow();
        let _x: i32 = *s1;
        let _y: i32 = *s2;
        let _x: i32 = *s1;
        let _y: i32 = *s2;
    }
    {
        let mut m = c.borrow_mut();
        let _z: i32 = *m;
        {
            let s: &i32 = &*m;
            let _x = *s;
        }
        *m = 23;
        let _z: i32 = *m;
    }
    {
        let s1 = c.borrow();
        let _x: i32 = *s1;
        let s2 = c.borrow();
        let _x: i32 = *s1;
        let _y: i32 = *s2;
        let _x: i32 = *s1;
        let _y: i32 = *s2;
    }
}

fn aliasing_mut_and_shr() {
    fn inner(rc: &RefCell<i32>, aliasing: &mut i32) {
        *aliasing += 4;
        let _escape_to_raw = rc as *const _;
        *aliasing += 4;
        let _shr = &*rc;
        *aliasing += 4;
    }

    let rc = RefCell::new(23);
    let mut bmut = rc.borrow_mut();
    inner(&rc, &mut *bmut);
    drop(bmut);
    assert_eq!(*rc.borrow(), 23+12);
}

fn main() {
    lots_of_funny_borrows();
    aliasing_mut_and_shr();
}
