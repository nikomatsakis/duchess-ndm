pub struct ClassDefinition {
    data: ClassDefinitionData,
}
enum ClassDefinitionData {
    Const {
        jni_class_name: &'static str,
        bytecode: &'static [i8],
    },
}

impl ClassDefinition {
    pub const fn new_const(jni_class_name: &'static str, bytecode: &'static [i8]) -> Self {
        Self {
            data: ClassDefinitionData::Const {
                jni_class_name,
                bytecode,
            },
        }
    }

    pub fn jni_class_name(&self) -> &str {
        match &self.data {
            ClassDefinitionData::Const {
                jni_class_name,
                bytecode: _,
            } => &jni_class_name,
        }
    }

    pub fn bytecode(&self) -> &[i8] {
        match &self.data {
            ClassDefinitionData::Const {
                jni_class_name: _,
                bytecode,
            } => bytecode,
        }
    }
}
