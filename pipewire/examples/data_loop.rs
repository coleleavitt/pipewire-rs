// pipewire/examples/data_loop.rs
use pipewire::{properties::Properties, loop_::DataLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize PipeWire
    pipewire::init();
    
    // Create a DataLoop with realtime properties
    let mut props = Properties::new()?;
    props.set("loop.class", "data.rt");
    props.set("loop.rt-prio", "88");
    
    let data_loop = DataLoop::new(Some(&props))?;
    println!("Created data loop: {:?}", data_loop.name());
    
    // Start the data loop thread
    data_loop.start()?;
    println!("In data loop thread? {}", data_loop.in_thread());
    
    // Run for a while
    std::thread::sleep(std::time::Duration::from_secs(1));
    
    // Exit and stop the data loop
    data_loop.exit();
    data_loop.stop()?;
    
    Ok(())
}

