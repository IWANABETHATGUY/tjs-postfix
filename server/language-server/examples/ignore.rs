use std::ops::Range;

fn main() {
    use ignore::Walk;
    let string = "我-".to_string();
    let ranges = get_word_range_of_string(&string);
    println!("{:?}", ranges);
    println!("{:?}", ranges.into_iter().map(|r| &string[r]).collect::<Vec<_>>());
    // for result in Walk::new("../../") {
    //     // Each item yielded by the iterator is either a directory entry or an
    //     // error, so either print the path or the error.
    //     match result {
    //         Ok(entry) => println!("{}", entry.path().display()),
    //         Err(err) => println!("ERROR: {}", err),
    //     }
    // }
}

fn get_word_range_of_string(string: &str) -> Vec<Range<usize>> {
    let mut index = -1;
    let mut iter = string.bytes().enumerate();
    let mut range = vec![];
    let mut in_word = false;
    while let Some((i, c)) = iter.next() {
        if c.is_ascii_whitespace() && index != -1 {
            in_word = false;
            range.push(index as usize..i);
            index = -1;
        } else if !c.is_ascii_whitespace() && index == -1 {
            in_word = true;
            index = i as i32;
        }
    }

    if in_word {
        range.push(index as usize..string.len())
    }
    range
}
