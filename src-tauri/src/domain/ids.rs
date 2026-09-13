use serde::{Deserialize, Serialize};

/// 强类型行 ID（newtype，防止 WordId/SenseId 等混用）。
/// 内部 i64 公开，便于 serde 与 rusqlite 在 store 层直接转换。
macro_rules! define_id {
	($($name:ident),* $(,)?) => {
		$(
			#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
			#[serde(transparent)]
			pub struct $name(pub i64);
		)*
	};
}

define_id!(WordId, SenseId, ExampleId, EncounterId, ReviewId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_transparently() {
        let json = serde_json::to_string(&WordId(42)).unwrap();
        assert_eq!(json, "42");
        let back: WordId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, WordId(42));
    }

    #[test]
    fn ids_do_not_mix() {
        // 编译期保证：把 WordId 传给要 SenseId 的地方无法编译。
        fn takes_sense(_: SenseId) {}
        takes_sense(SenseId(1));
    }
}
