//! 数据模型：一张便签与内存中的便签集合。
//! 数据库按「一条便签一个 key」存储，`Notes` 只作为 UI 侧的只读/可写缓存，
//! 同时保留 serde 派生，供旧版 notes.json 迁移使用。

use serde::{Deserialize, Serialize};

use crate::util::now_secs;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub color: usize,
    pub created: u64,
    pub updated: u64,
}

impl Note {
    pub fn new(id: u64, color: usize) -> Self {
        let now = now_secs();
        Self {
            id,
            title: String::new(),
            content: String::new(),
            color,
            created: now,
            updated: now,
        }
    }

    /// 列表卡片上的标题：空标题显示「无标题」。
    pub fn display_title(&self) -> String {
        if self.title.trim().is_empty() {
            "无标题".to_owned()
        } else {
            self.title.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Notes {
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub next_id: u64,
    /// 上次关闭时正在查看的便签 id，用于启动恢复。
    #[serde(default)]
    pub last_open: Option<u64>,
}

impl Notes {
    pub fn get(&self, id: u64) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    /// 按更新时间倒序返回 id 列表（可先按关键字过滤）。
    pub fn sorted_ids(&self, query: &str) -> Vec<u64> {
        let q = query.trim().to_lowercase();
        let mut ids: Vec<u64> = self
            .notes
            .iter()
            .filter(|n| {
                q.is_empty()
                    || n.title.to_lowercase().contains(&q)
                    || n.content.to_lowercase().contains(&q)
            })
            .map(|n| n.id)
            .collect();
        ids.sort_by(|a, b| {
            let ua = self.get(*a).map_or(0, |n| n.updated);
            let ub = self.get(*b).map_or(0, |n| n.updated);
            ub.cmp(&ua)
        });
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_ids_by_updated_desc() {
        let mut notes = Notes::default();
        notes.notes.push(Note {
            updated: 10,
            ..Note::new(1, 0)
        });
        notes.notes.push(Note {
            updated: 30,
            ..Note::new(2, 1)
        });
        notes.notes.push(Note {
            updated: 20,
            ..Note::new(3, 2)
        });
        assert_eq!(notes.sorted_ids(""), vec![2, 3, 1]);
        assert_eq!(notes.sorted_ids("不存在"), Vec::<u64>::new());
    }
}
