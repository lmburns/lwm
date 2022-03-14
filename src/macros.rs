//! Macros for error/warning printing

/// Expand to an error message
#[macro_export]
macro_rules! lwm_error {
    ($($err:tt)*) => ({
        use colored::Colorize;
        eprintln!("{}: {}", "[lwm error]".red().bold(), format!($($err)*));
    })
}

/// Expand to an info message
#[macro_export]
macro_rules! lwm_info {
    ($($err:tt)*) => ({
        use colored::Colorize;
        eprintln!("{}: {}", "[lwm info]".purple().bold(), format!($($err)*));
    })
}

/// Expand to a fatal message
#[macro_export]
macro_rules! lwm_fatal {
    ($($err:tt)*) => ({
        use colored::Colorize;
        eprintln!("{}: {}", "[lwm fatal]".yellow().bold(), format!($($err)*));
        std::process::exit(1);
    })
}

/// Create a [`HashMap`](std::collections::HashMap) easily
#[macro_export]
macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key.to_owned(), $val); )*
         map
    }}
}
