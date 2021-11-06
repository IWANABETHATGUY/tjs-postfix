use std::path::Path;

use crossbeam_channel::unbounded;
use notify::{Config, RecommendedWatcher, RecursiveMode, Result, Watcher};

#[tokio::main]
async fn main() -> Result<()> {
    // Automatically select the best implementation for your platform.
    // let mut watcher = notify::recommended_watcher(|res| {
    //     match res {
    //        Ok(event) => println!("event: {:?}", event),
    //        Err(e) => println!("watch error: {:?}", e),
    //     }
    // })?;
    let result = tokio::task::spawn_blocking(move || -> Result<()> {
        let (tx, rx) = unbounded();
        let mut watcher = RecommendedWatcher::new(move |e| match e {
            Ok(e) => {
                tx.send(e).unwrap();
            }
            Err(err) => {}
        })?;
        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(Path::new("/home/hxj/Documents/css/github/grid-flexbox-v2"), RecursiveMode::Recursive)?;
        watcher.configure(Config::NoticeEvents(true))?;
        loop {
            match rx.recv() {
                Ok(e) => {
                    println!("{:?}", e);
                    // let res = e.paths.iter().filter_map(|p| std::fs::canonicalize(p).ok()).collect::<Vec<_>>();
                    // println!("{:?}", res);
                }
                Err(_) => todo!(),
            }
        }
    });
    // result.join().unwrap().unwrap();
    Ok(())
}
