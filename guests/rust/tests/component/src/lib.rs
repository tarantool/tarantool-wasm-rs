use tarantool_wasm::{msgpack, Error, IteratorType, Space, Tuple, Value};

wit_bindgen::generate!({ path: "wit", world: "sdk-test" });

struct Component;

impl Guest for Component {
    fn check() -> String {
        let space = Space::by_name("rust_users").unwrap().unwrap();
        let primary = space.primary();
        assert_eq!(space.index_by_name("primary").unwrap().unwrap().id(), 0);
        assert!(space.index_by_name("missing").unwrap().is_none());
        assert!(Space::by_name("missing_sdk_space").unwrap().is_none());

        let tuple = space.insert(&(1u64, "Rust")).unwrap();
        let retained = tuple.clone();
        drop(tuple);

        assert_eq!(
            retained.decode::<(u64, String)>().unwrap(),
            (1, "Rust".into())
        );
        assert_eq!(retained.field::<String>(1).unwrap(), Some("Rust".into()));
        assert_eq!(retained.field::<Value>(99).unwrap(), None);
        let result = space.insert(&(1u64, "duplicate"));
        assert!(matches!(result, Err(Error::Host(error)) if error.code == 3));
        assert!(primary.get(&(99u64,)).unwrap().is_none());

        drop(
            primary
                .update(&(1u64,), &[("=", 1, "updated")])
                .unwrap()
                .unwrap(),
        );
        assert_eq!(
            primary
                .get(&(1u64,))
                .unwrap()
                .unwrap()
                .field::<String>(1)
                .unwrap(),
            Some("updated".into())
        );

        drop(space.replace(&(2u64, "second")).unwrap());
        space
            .upsert(&(2u64, "ignored"), &[("=", 1, "upserted")])
            .unwrap();
        assert_eq!(
            primary
                .get(&(2u64,))
                .unwrap()
                .unwrap()
                .decode::<(u64, String)>()
                .unwrap(),
            (2, "upserted".into())
        );

        assert_eq!(primary.len().unwrap(), 2);
        assert_eq!(
            primary.count(&Vec::<u8>::new(), IteratorType::All).unwrap(),
            2
        );
        assert_eq!(
            primary
                .scan(&Vec::<u8>::new(), IteratorType::All)
                .unwrap()
                .count(),
            2
        );
        {
            let mut scan = primary.scan(&Vec::<u8>::new(), IteratorType::All).unwrap();
            drop(scan.next().unwrap().unwrap());
        }
        let mut empty = primary.scan(&(99u64,), IteratorType::Equal).unwrap();
        assert!(empty.next().is_none());
        assert!(empty.next().is_none());

        drop(primary.delete(&(2u64,)).unwrap());
        let local = Tuple::new(&(None::<u32>,)).unwrap();
        assert_eq!(local.field::<Option<u32>>(0).unwrap(), Some(None));

        space.truncate().unwrap();
        assert!(primary.is_empty().unwrap());

        "ok".into()
    }

    fn roundtrip(data: Vec<u8>) -> Vec<u8> {
        fn convert(value: Value) -> Value {
            match value {
                Value::Ext(tag, _) => match tag {
                    1 => msgpack::Decimal::try_from(value)
                        .unwrap()
                        .to_value()
                        .unwrap(),
                    2 => msgpack::Uuid::try_from(value).unwrap().to_value().unwrap(),
                    3 => msgpack::ErrorValue::try_from(value)
                        .unwrap()
                        .to_value()
                        .unwrap(),
                    4 => msgpack::Datetime::try_from(value)
                        .unwrap()
                        .to_value()
                        .unwrap(),
                    6 => msgpack::Interval::try_from(value)
                        .unwrap()
                        .to_value()
                        .unwrap(),
                    _ => value,
                },
                Value::Array(items) => Value::Array(items.into_iter().map(convert).collect()),
                Value::Map(items) => Value::Map(
                    items
                        .into_iter()
                        .map(|(k, v)| (convert(k), convert(v)))
                        .collect(),
                ),
                _ => value,
            }
        }
        let value: Value = msgpack::unpack(&data).unwrap();
        msgpack::pack(&convert(value)).unwrap()
    }

    fn exchange(data: Vec<u8>) -> Vec<u8> {
        let value: Value = msgpack::unpack(&Self::roundtrip(data)).unwrap();
        let space = Space::by_name("rust_specials").unwrap().unwrap();
        drop(space.replace(&value).unwrap());
        space
            .primary()
            .get(&(1u64,))
            .unwrap()
            .unwrap()
            .to_vec()
            .unwrap()
    }
}

export!(Component);
