// Copyright (c) 2025 Nobuharu Shimazu
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Storage-class specifiers for functions and variables.

/// A C storage-class specifier.
///
/// `static` and `extern` are mutually exclusive in C — a declaration can carry
/// at most one of them — so they are modeled as a single enum rather than two
/// independent booleans. That makes the invalid `extern static` combination
/// unrepresentable instead of merely discouraged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageClass {
    /// No storage-class specifier (the default: external linkage for file-scope
    /// definitions, automatic for locals).
    #[default]
    None,

    /// `static` — internal linkage at file scope, or a static local
    Static,

    /// `extern` — external linkage; typically a declaration whose definition
    /// lives in another translation unit
    Extern,
}

impl StorageClass {
    /// Returns the C keyword for this storage class, or `None` for
    /// [`StorageClass::None`]. The returned keyword does not include a trailing
    /// space
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            StorageClass::None => None,
            StorageClass::Static => Some("static"),
            StorageClass::Extern => Some("extern"),
        }
    }

    /// Whether this is [`StorageClass::Extern`].
    pub fn is_extern(self) -> bool {
        self == StorageClass::Extern
    }

    /// Whether this is [`StorageClass::Static`].
    pub fn is_static(self) -> bool {
        self == StorageClass::Static
    }
}
