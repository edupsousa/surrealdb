// Tests common to all protocols and storage engines

#[tokio::test]
async fn connect() {
	let db = new_db().await;
	db.health().await.unwrap();
}

#[tokio::test]
async fn yuse() {
	let db = new_db().await;
    let item = Ulid::new().to_string();
    let error = db.create::<Vec<()>>(item.as_str()).await.unwrap_err();
    match error {
        // Local engines return this error
        Error::Db(DbError::NsEmpty) => {}
        // Remote engines return this error
        Error::Api(ApiError::Query(error)) if error.contains("Specify a namespace to use") => {}
        error => panic!("{:?}", error),
    }
	db.use_ns(NS).use_db(item).await.unwrap();
}

#[tokio::test]
async fn query() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let _ = db.query("
        CREATE user:john
        SET name = 'John Doe'
    ")
    .await
    .unwrap()
    .check()
    .unwrap();
	let mut response = db
        .query("SELECT name FROM user:john")
        .await
        .unwrap()
        .check()
        .unwrap();
    let Some(name): Option<String> = response.take("name").unwrap() else {
        panic!("query returned no record");
    };
    assert_eq!(name, "John Doe");
}

#[tokio::test]
async fn query_binds() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let mut response = db.query("CREATE user:john SET name = $name")
        .bind(("name", "John Doe"))
        .await
        .unwrap();
    let Some(record): Option<RecordName> = response.take(0).unwrap() else {
        panic!("query returned no record");
    };
    assert_eq!(record.name, "John Doe");
	let mut response = db.query("SELECT * FROM $record_id")
        .bind(("record_id", thing("user:john").unwrap()))
        .await
        .unwrap();
    let Some(record): Option<RecordName> = response.take(0).unwrap() else {
        panic!("query returned no record");
    };
    assert_eq!(record.name, "John Doe");
	let mut response = db.query("CREATE user SET name = $name")
		.bind(Record {
			name: "John Doe",
		})
		.await
		.unwrap();
    let Some(record): Option<RecordName> = response.take(0).unwrap() else {
        panic!("query returned no record");
    };
    assert_eq!(record.name, "John Doe");
}

#[tokio::test]
async fn query_chaining() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let response = db
        .query(BeginStatement)
		.query("CREATE account:one SET balance = 135605.16")
		.query("CREATE account:two SET balance = 91031.31")
		.query("UPDATE account:one SET balance += 300.00")
		.query("UPDATE account:two SET balance -= 300.00")
		.query(CommitStatement)
		.await
		.unwrap();
    response.check().unwrap();
}

#[tokio::test]
async fn create_record_no_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let _: Vec<RecordId> = db.create("user").await.unwrap();
	let _: Value = db.create(Resource::from("user")).await.unwrap();
}

#[tokio::test]
async fn create_record_with_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let _: Option<RecordId> = db.create(("user", "jane")).await.unwrap();
	let _: Value = db.create(Resource::from(("user", "john"))).await.unwrap();
	let _: Value = db.create(Resource::from("user:doe")).await.unwrap();
}

#[tokio::test]
async fn create_record_no_id_with_content() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let _: Vec<RecordId> = db
		.create("user")
		.content(Record {
			name: "John Doe",
		})
		.await
		.unwrap();
	let _: Value = db
		.create(Resource::from("user"))
		.content(Record {
			name: "John Doe",
		})
		.await
		.unwrap();
}

#[tokio::test]
async fn create_record_with_id_with_content() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let record: Option<RecordId> = db
		.create(("user", "john"))
		.content(Record {
			name: "John Doe",
		})
		.await
		.unwrap();
	assert_eq!(record.unwrap().id, thing("user:john").unwrap());
	let value: Value = db
		.create(Resource::from("user:jane"))
		.content(Record {
			name: "Jane Doe",
		})
		.await
		.unwrap();
	assert_eq!(value.record(), thing("user:jane").ok());
}

#[tokio::test]
async fn select_table() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let table = "user";
	let _: Vec<RecordId> = db.create(table).await.unwrap();
	let _: Vec<RecordId> = db.create(table).await.unwrap();
	let _: Value = db.create(Resource::from(table)).await.unwrap();
	let users: Vec<RecordId> = db.select(table).await.unwrap();
    assert_eq!(users.len(), 3);
}

#[tokio::test]
async fn select_record_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let record_id = ("user", "john");
	let _: Option<RecordId> = db.create(record_id).await.unwrap();
	let Some(record): Option<RecordId> = db.select(record_id).await.unwrap() else {
        panic!("record not found");
    };
    assert_eq!(record.id, thing("user:john").unwrap());
	let value: Value = db.select(Resource::from(record_id)).await.unwrap();
    assert_eq!(value.record(), thing("user:john").ok());
}

#[tokio::test]
async fn select_record_ranges() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let table = "user";
	let _: Option<RecordId> = db.create((table, "amos")).await.unwrap();
	let _: Option<RecordId> = db.create((table, "jane")).await.unwrap();
	let _: Option<RecordId> = db.create((table, "john")).await.unwrap();
	let _: Value = db.create(Resource::from((table, "zoey"))).await.unwrap();
	let convert = |users: Vec<RecordId>| -> Vec<String> {
		users
			.into_iter()
			.map(|user| user.id.id.to_string())
			.collect()
	};
	let users: Vec<RecordId> = db.select(table).range(..).await.unwrap();
	assert_eq!(convert(users), vec!["amos", "jane", "john", "zoey"]);
	let users: Vec<RecordId> = db.select(table).range(.."john").await.unwrap();
	assert_eq!(convert(users), vec!["amos", "jane"]);
	let users: Vec<RecordId> = db.select(table).range(..="john").await.unwrap();
	assert_eq!(convert(users), vec!["amos", "jane", "john"]);
	let users: Vec<RecordId> = db.select(table).range("jane"..).await.unwrap();
	assert_eq!(convert(users), vec!["jane", "john", "zoey"]);
	let users: Vec<RecordId> = db.select(table).range("jane".."john").await.unwrap();
	assert_eq!(convert(users), vec!["jane"]);
	let users: Vec<RecordId> = db.select(table).range("jane"..="john").await.unwrap();
	assert_eq!(convert(users), vec!["jane", "john"]);
	let Value::Array(array): Value = db.select(Resource::from(table)).range("jane"..="john").await.unwrap() else {
        unreachable!();
    };
	assert_eq!(array.len(), 2);
	let users: Vec<RecordId> =
		db.select(table).range((Bound::Excluded("jane"), Bound::Included("john"))).await.unwrap();
	assert_eq!(convert(users), vec!["john"]);
}

#[tokio::test]
async fn update_table() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let table = "user";
    let _: Vec<RecordId> = db.create(table).await.unwrap();
    let _: Vec<RecordId> = db.create(table).await.unwrap();
	let _: Value = db.update(Resource::from(table)).await.unwrap();
	let users: Vec<RecordId> = db.update(table).await.unwrap();
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn update_record_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let table = "user";
    let _: Option<RecordId> = db.create((table, "john")).await.unwrap();
    let _: Option<RecordId> = db.create((table, "jane")).await.unwrap();
	let users: Vec<RecordId> = db.update(table).await.unwrap();
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn update_table_with_content() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let sql = "
        CREATE type::thing($table, 'amos') SET name = 'Amos';
        CREATE type::thing($table, 'jane') SET name = 'Jane';
        CREATE type::thing($table, 'john') SET name = 'John';
        CREATE type::thing($table, 'zoey') SET name = 'Zoey';
    ";
	let table = "user";
    let response = db.query(sql)
        .bind(("table", table))
        .await
        .unwrap();
    response.check().unwrap();
	let users: Vec<RecordBuf> = db
		.update(table)
		.content(Record {
			name: "Doe",
		})
		.await
		.unwrap();
    let expected = &[
        RecordBuf {
            id: thing("user:amos").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:jane").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:john").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:zoey").unwrap(),
            name: "Doe".to_owned(),
        },
    ];
    assert_eq!(users, expected);
	let users: Vec<RecordBuf> = db
		.select(table)
		.await
		.unwrap();
    assert_eq!(users, expected);
}

#[tokio::test]
async fn update_record_range_with_content() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let sql = "
        CREATE type::thing($table, 'amos') SET name = 'Amos';
        CREATE type::thing($table, 'jane') SET name = 'Jane';
        CREATE type::thing($table, 'john') SET name = 'John';
        CREATE type::thing($table, 'zoey') SET name = 'Zoey';
    ";
	let table = "user";
    let response = db.query(sql)
        .bind(("table", table))
        .await
        .unwrap();
    response.check().unwrap();
	let users: Vec<RecordBuf> = db
		.update(table)
		.range("jane".."zoey")
		.content(Record {
			name: "Doe",
		})
		.await
		.unwrap();
    assert_eq!(users, &[
        RecordBuf {
            id: thing("user:jane").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:john").unwrap(),
            name: "Doe".to_owned(),
        },
    ]);
	let users: Vec<RecordBuf> = db
		.select(table)
		.await
		.unwrap();
    assert_eq!(users, &[
        RecordBuf {
            id: thing("user:amos").unwrap(),
            name: "Amos".to_owned(),
        },
        RecordBuf {
            id: thing("user:jane").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:john").unwrap(),
            name: "Doe".to_owned(),
        },
        RecordBuf {
            id: thing("user:zoey").unwrap(),
            name: "Zoey".to_owned(),
        },
    ]);
}

#[tokio::test]
async fn update_record_id_with_content() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let record_id = ("user", "john");
	let user: Option<RecordName> = db
		.create(record_id)
		.content(Record {
			name: "Jane Doe",
		})
		.await
		.unwrap();
    assert_eq!(user.unwrap().name, "Jane Doe");
	let user: Option<RecordName> = db
		.update(record_id)
		.content(Record {
			name: "John Doe",
		})
		.await
		.unwrap();
    assert_eq!(user.unwrap().name, "John Doe");
	let user: Option<RecordName> = db
		.select(record_id)
		.await
		.unwrap();
    assert_eq!(user.unwrap().name, "John Doe");
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
struct Name {
    first: Cow<'static, str>,
    last: Cow<'static, str>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
struct Person {
    #[serde(skip_serializing)]
    id: Option<Thing>,
    title: Cow<'static, str>,
    name: Name,
    marketing: bool,
}

#[tokio::test]
async fn merge_record_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let record_id = ("person", "jaime");
    let mut jaime: Option<Person> = db
        .create(record_id)
        .content(Person {
            id: None,
            title: "Founder & COO".into(),
            name: Name {
                first: "Jaime".into(),
                last: "Morgan Hitchcock".into(),
            },
            marketing: false,
        })
        .await
        .unwrap();
    assert_eq!(jaime.unwrap().id.unwrap(), thing("person:jaime").unwrap());
    jaime = db
        .update(record_id)
        .merge(json!({ "marketing": true }))
        .await
        .unwrap();
    assert!(jaime.as_ref().unwrap().marketing);
    jaime = db.select(record_id).await.unwrap();
    assert_eq!(jaime.unwrap(), Person {
        id: Some(thing("person:jaime").unwrap()),
        title: "Founder & COO".into(),
        name: Name {
            first: "Jaime".into(),
            last: "Morgan Hitchcock".into(),
        },
        marketing: true,
    });
}

#[tokio::test]
async fn patch_record_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
	let id = "john";
	let _: Option<RecordId> = db
		.create(("user", id))
		.content(json!({
			"baz": "qux",
			"foo": "bar"
		}))
		.await
		.unwrap();
	let _: Option<serde_json::Value> = db
		.update(("user", id))
		.patch(PatchOp::replace("/baz", "boo"))
		.patch(PatchOp::add("/hello", ["world"]))
		.patch(PatchOp::remove("/foo"))
		.await
		.unwrap();
	let value: Option<serde_json::Value> = db.select(("user", id)).await.unwrap();
	assert_eq!(
		value,
		Some(json!({
			"id": thing(&format!("user:{id}")).unwrap(),
			"baz": "boo",
			"hello": ["world"]
		}))
	);
}

#[tokio::test]
async fn delete_table() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let table = "user";
	let _: Vec<RecordId> = db.create(table).await.unwrap();
	let _: Vec<RecordId> = db.create(table).await.unwrap();
	let _: Vec<RecordId> = db.create(table).await.unwrap();
    let users: Vec<RecordId> = db.select(table).await.unwrap();
    assert_eq!(users.len(), 3);
	let users: Vec<RecordId> = db.delete(table).await.unwrap();
    assert_eq!(users.len(), 3);
    let users: Vec<RecordId> = db.select(table).await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn delete_record_id() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let record_id = ("user", "john");
	let _: Option<RecordId> = db.create(record_id).await.unwrap();
    let _: Option<RecordId> = db.select(record_id).await.unwrap();
	let john: Option<RecordId> = db.delete(record_id).await.unwrap();
    assert!(john.is_some());
    let john: Option<RecordId> = db.select(record_id).await.unwrap();
    assert!(john.is_none());
    // non-existing user
	let jane: Option<RecordId> = db.delete(("user", "jane")).await.unwrap();
    assert!(jane.is_none());
	let value = db.delete(Resource::from(("user", "jane"))).await.unwrap();
    assert_eq!(value, Value::None);
}

#[tokio::test]
async fn delete_record_range() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let sql = "
        CREATE type::thing($table, 'amos') SET name = 'Amos';
        CREATE type::thing($table, 'jane') SET name = 'Jane';
        CREATE type::thing($table, 'john') SET name = 'John';
        CREATE type::thing($table, 'zoey') SET name = 'Zoey';
    ";
	let table = "user";
    let response = db.query(sql)
        .bind(("table", table))
        .await
        .unwrap();
    response.check().unwrap();
	let users: Vec<RecordBuf> = db.delete(table).range("jane".."zoey").await.unwrap();
    assert_eq!(users, &[
        RecordBuf {
            id: thing("user:jane").unwrap(),
            name: "Jane".to_owned(),
        },
        RecordBuf {
            id: thing("user:john").unwrap(),
            name: "John".to_owned(),
        },
    ]);
	let users: Vec<RecordBuf> = db
		.select(table)
		.await
		.unwrap();
    assert_eq!(users, &[
        RecordBuf {
            id: thing("user:amos").unwrap(),
            name: "Amos".to_owned(),
        },
        RecordBuf {
            id: thing("user:zoey").unwrap(),
            name: "Zoey".to_owned(),
        },
    ]);
}

#[tokio::test]
async fn version() {
	let db = new_db().await;
	db.version().await.unwrap();
}

#[tokio::test]
async fn set_unset() {
	let db = new_db().await;
	db.use_ns(NS).use_db(Ulid::new().to_string()).await.unwrap();
    let (key, value) = ("name", "Doe");
    let sql = "RETURN $name";
	db.set(key, value).await.unwrap();
    let mut response = db.query(sql).await.unwrap();
    let Some(name): Option<String> = response.take(0).unwrap() else {
        panic!("record not found");
    };
    assert_eq!(name, value);
	db.unset(key).await.unwrap();
    let mut response = db.query(sql).await.unwrap();
    let name: Option<String> = response.take(0).unwrap();
    assert!(name.is_none());
}

#[tokio::test]
async fn return_bool() {
	let db = new_db().await;
	let mut response = db.query("RETURN true").await.unwrap();
    let Some(boolean): Option<bool> = response.take(0).unwrap() else {
        panic!("record not found");
    };
    assert!(boolean);
	let mut response = db.query("RETURN false").await.unwrap();
    let value: Value = response.take(0).unwrap();
    assert_eq!(value, vec![Value::Bool(false)].into());
}
