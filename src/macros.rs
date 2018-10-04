macro_rules! ectx {
    (err_contexts $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let mut msg = "at ".to_string();
        msg.push_str(&format!("{}:{}", file!(), line!()));
        $(
            $(
                let arg = format!("\nwith args - {}: {:#?}", stringify!($arg), $arg);
                msg.push_str(&arg);
            )*
        )*
        let err = $e.context(msg);
        $(
            let err = err.context($context);
        )*
        err
    }};

    (err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let err = ectx!(err_contexts $e $(,$context)* $(=> $($arg),*)*);
        err.into()
    }};

    (catch err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let e = $e.kind().into();
        ectx!(err $e $(,$context)*, e $(=> $($arg),*)*)
    }};

    (catch $($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(catch err e $(,$context)* $(=> $($arg),*)*)
        }
    }};

    (raw_err $($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(err_contexts e $(,$context)* $(=> $($arg),*)*)
        }
    }};

    ($($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(err e $(,$context)* $(=> $($arg),*)*)
        }
    }};
}
