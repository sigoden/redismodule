use iredismodule::key::{HashGetFlag, HashSetFlag, KeyType, ListPosition, ZsetRangeDirection};
use iredismodule::prelude::*;
use iredismodule_macros::{rcmd, rwrap};
use rand::random;
use std::time::Duration;

#[rcmd("hello.simple")]
fn hello_simple(ctx: &mut Context, _args: Vec<RStr>) -> RResult {
    let db = ctx.get_select_db();
    Ok(db.into())
}

#[rcmd("hello.push.native", "write deny-oom", 1, 1, 1)]
fn hello_push_native(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 3);
    let key = ctx.open_write_key(&args[1]);
    key.list_push(ListPosition::Tail, &args[2])?;
    let len = key.value_length();
    Ok(len.into())
}

#[rcmd("hello.push.call", "write deny-oom", 1, 1, 1)]
fn hello_push_call(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 3);
    let call_args: Vec<&RStr> = args.iter().skip(1).collect();
    ctx.call("RPUSH", CallFlags::None, &call_args)
        .unwrap()
        .into()
}

#[rcmd("hello.push.call2", "write deny-oom", 1, 1, 1)]
fn hello_push_call2(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    hello_push_call(ctx, args)
}

#[rcmd("hello.push.sum.len", "readonly", 1, 1, 1)]
fn hello_list_sum_len(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 2);
    let call_args = [&args[1].to_str()?, "0", "-1"];
    let reply = ctx.call_str("LRANGE", CallFlags::None, &call_args)?;

    let elem_len = reply.get_length();
    let str_len: usize = (0..elem_len)
        .map(|v| reply.get_array_element(v).unwrap().get_length())
        .sum();
    Ok(Value::from(str_len))
}

#[rcmd("hello.list.splice", "write deny-oom", 1, 2, 1)]
fn hello_list_splice(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 4);
    let mut src_key = ctx.open_write_key(&args[1]);
    let mut dest_key = ctx.open_write_key(&args[2]);
    src_key.verify_type(KeyType::List, true)?;
    dest_key.verify_type(KeyType::List, true)?;
    let count = args[3]
        .assert_integer(|v| v > 0)
        .map_err(|_| Error::new("ERR invalid count"))?;
    for _ in 0..count {
        let ele = src_key.list_pop(ListPosition::Tail);
        match ele {
            Err(_) => break,
            Ok(v) => {
                dest_key.list_push(ListPosition::Head, &v)?;
            }
        }
    }
    let len = src_key.value_length();
    Ok(len.into())
}

#[rcmd("hello.list.splice.auto", "write deny-oom", 1, 2, 1)]
fn hello_list_splice_auto(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    hello_list_splice(ctx, args)
}

#[rcmd("hello.rand.array")]
fn hello_rand_array(_ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 2);
    let count = args[1]
        .assert_integer(|v| v > 0)
        .map_err(|_| Error::new("ERR invalid count"))?;
    let value: Vec<Value> = (0..count).map(|_| random::<i64>().into()).collect();
    Ok(Value::Array(value))
}

#[rcmd("hello.repl1")]
fn hello_repl1(ctx: &mut Context, _args: Vec<RStr>) -> RResult {
    ctx.replicate_str("ECHO", CallFlags::None, &["foo"])?;
    ctx.call_str("INCR", CallFlags::None, &["foo"])?;
    ctx.call_str("INCR", CallFlags::None, &["bar"])?;
    Ok(0i64.into())
}

#[rcmd("hello.repl2", "write", 1, 1, 1)]
fn hello_repl2(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 2);
    let key = ctx.open_write_key(&args[1]);
    key.verify_type(KeyType::List, false)?;
    let list_len = key.value_length();
    let mut sum = 0;
    for _ in 0..list_len {
        let ele = key.list_pop(ListPosition::Tail)?;
        let mut val = ele.get_integer().unwrap_or(0);
        val += 1;
        sum += val;
        let new_ele = RString::from_str(&val.to_string());
        key.list_push(ListPosition::Head, &new_ele)?;
    }
    ctx.replicate_verbatim();
    Ok(sum.into())
}

#[rcmd("hello.toggle.case", "write", 1, 1, 1)]
fn hello_toggle_case(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 2);
    let key = ctx.open_write_key(&args[1]);
    key.verify_type(KeyType::String, true)?;
    if key.get_type() == KeyType::String {
        let value = key.string_get()?;
        let value = value
            .to_str()?
            .chars()
            .map(|v| {
                if v.is_ascii_uppercase() {
                    v.to_ascii_lowercase()
                } else {
                    v.to_ascii_uppercase()
                }
            })
            .collect::<String>();
        key.string_set(&RString::from_str(&value))?;
    }
    ctx.replicate_verbatim();
    Ok("OK".into())
}

#[rcmd("hello.more.expire", "write", 1, 1, 1)]
fn hello_more_expire(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 3);
    let addms = args[2]
        .get_integer()
        .map_err(|_e| Error::new("ERR invalid expire time"))?;
    let key = ctx.open_write_key(&args[1]);
    let expire = key.get_expire();
    if let Some(d) = expire {
        ctx.debug(&format!("current duration {}", d.as_secs()));
        let new_d = d.checked_add(Duration::from_millis(addms as u64)).unwrap();
        key.set_expire(new_d)?;
    } else {
        ctx.debug(&format!("current no duration"));
    }
    Ok("OK".into())
}

#[rcmd("hello.zsumrange", "readonly", 1, 1, 1)]
fn hello_zsumrange(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 4);
    let key = ctx.open_write_key(&args[1]);
    key.verify_type(KeyType::ZSet, false)?;
    let tail_args = args
        .iter()
        .skip(2)
        .map(|v| v.get_integer())
        .collect::<Result<Vec<i64>, Error>>()
        .map_err(|_e| Error::new("invalid range"))?;
    let score_start = tail_args[0] as f64;
    let score_end = tail_args[1] as f64;
    let v1 = key.zset_score_range(
        ZsetRangeDirection::FristIn,
        score_start,
        score_end,
        false,
        false,
    )?;
    let v2 = key.zset_score_range(
        ZsetRangeDirection::LastIn,
        score_start,
        score_end,
        false,
        false,
    )?;
    let score1: f64 = v1.iter().map(|v| v.1).sum();
    let score2: f64 = v2.iter().map(|v| v.1).sum();
    let result: Vec<Value> = vec![score1.into(), score2.into()];
    Ok(Value::Array(result))
}

#[rcmd("hello.lexrange", "readonly", 1, 1, 1)]
fn hello_lexrange(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 4);
    let key = ctx.open_write_key(&args[1]);
    key.verify_type(KeyType::ZSet, false)?;
    let v = key.zset_lex_range(ZsetRangeDirection::FristIn, &args[2], &args[3])?;
    let result: Vec<Value> = v
        .iter()
        .map(|v| v.0.to_str().unwrap().to_owned().into())
        .collect();
    Ok(Value::Array(result))
}

#[rcmd("hello.hcopy", "write deny-oom", 1, 1, 1)]
fn hello_hcopy(ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 4);
    let key = ctx.open_write_key(&args[1]);
    key.verify_type(KeyType::ZSet, true)?;
    let old_val = key.hash_get(HashGetFlag::None, &args[2])?;
    if let Some(v) = &old_val {
        ctx.debug(&format!("old_val is {}", v.to_str()?));
        key.hash_set(HashSetFlag::None, &args[3], Some(v))?;
        ctx.debug(&format!("new_val is {}", v.to_str()?));
    }
    let ret: i64 = match &old_val {
        Some(_) => 1,
        None => 0,
    };
    Ok(ret.into())
}

#[rcmd("hello.leftpad", "", 0, 0, 0)]
fn hello_leftpad(_ctx: &mut Context, args: Vec<RStr>) -> RResult {
    assert_len!(args, 4);
    let pad_len = args[2]
        .assert_integer(|v| v > 0)
        .map_err(|_| Error::new("ERR invalid padding length"))? as usize;
    let the_str: &str = args[1].to_str()?;
    let the_char: &str = args[3].to_str()?;
    if the_str.len() >= pad_len {
        return Ok(the_str.into());
    }
    if the_char.len() != 1 {
        return Err(Error::new("ERR padding must be a single char"));
    }
    let the_char = the_char.chars().nth(0).unwrap();
    let mut pad_str = (0..(pad_len - the_str.len()))
        .map(|_| the_char)
        .collect::<String>();
    pad_str.push_str(the_str);
    Ok(pad_str.into())
}

#[rwrap("call")]
fn init(ctx: &mut Context, args: Vec<RStr>) -> Result<(), Error> {
    ctx.debug(&format!(
        "Module loaded with ARGV[{}] = {:?}\n",
        args.len(),
        args.iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect::<Vec<String>>()
    ));
    Ok(())
}

define_module! {
    name: "hello",
    version: 1,
    data_types: [],
    init_funcs: [
        init_c,
    ],
    commands: [
        hello_simple_cmd,
        hello_push_native_cmd,
        hello_push_call_cmd,
        hello_push_call2_cmd,
        hello_list_sum_len_cmd,
        hello_list_splice_cmd,
        hello_list_splice_auto_cmd,
        hello_rand_array_cmd,
        hello_repl1_cmd,
        hello_repl2_cmd,
        hello_toggle_case_cmd,
        hello_more_expire_cmd,
        hello_zsumrange_cmd,
        hello_lexrange_cmd,
        hello_hcopy_cmd,
        hello_leftpad_cmd,
    ],
}
