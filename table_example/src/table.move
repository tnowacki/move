module Std::Table {
    use Std::Option::{Self, Option};

    struct Table<K: copy + drop + store, V: store> has store { /* silence warnings */ k: K, v: V }

    native public fun create<K: copy + drop + store, V: store>(_s: &signer):  Table<K, V>;
    native public fun destroy_empty<K: copy + drop + store, V: store>(_t: Table<K, V>);

    native public fun is_empty<K: copy + drop + store, V: store>(_t: &Table<K, V>): bool;
    native public fun has_key<K: copy + drop + store, V: store>(_t: &Table<K, V>, _k: &K): bool;

    native public fun borrow<K: copy + drop + store, V: store>(_t: &Table<K, V>, _k: &K): &V;
    native public fun borrow_mut<K: copy + drop + store, V: store>(_t: &mut Table<K, V>, _k: &K): &mut V;

    native public fun insert_or_abort<K: copy + drop + store, V: store>(_t: &mut Table<K, V>, _k: K, _v: V);
    native public fun remove_or_abort<K: copy + drop + store, V: store>(_t: &mut Table<K, V>, _k: &K): V;

    public fun insert<K: copy + drop + store, V: store>(t: &mut Table<K, V>, k: K, v: V): Option<V> {
        let prev = remove(t, &k);
        insert_or_abort(t, k, v);
        prev
    }

    public fun remove<K: copy + drop + store, V: store>(t: &mut Table<K, V>, k: &K): Option<V> {
        if (has_key(t, k)) Option::some(remove_or_abort(t, k)) else Option::none()
    }
}

module Std::LinkedTable {
    use Std::Option::{Self, Option};
    use Std::Table::{Self, Table};

    struct LinkedTable<K: copy + drop + store, V: store> has store {
        inner: Table<K, Node<K, V>>,
        head: Option<K>,
        tail: Option<K>,
    }
    struct Node<K: copy + drop + store, V: store> has copy, drop, store {
        value: V,
        prev: Option<K>,
        next: Option<K>,
    }

    public fun create<K: copy + drop + store, V: store>(s: &signer):  LinkedTable<K, V> {
        LinkedTable {
            inner: Table::create(s),
            head: Option::none(),
            tail: Option::none(),
        }
    }
    public fun destroy_empty<K: copy + drop + store, V: store>(t: LinkedTable<K, V>) {
        let LinkedTable { inner, head: _, tail: _ } = t;
        Table::destroy_empty(inner)
    }
    public fun head<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>): Option<K> {
        *&t.head
    }
    public fun tail<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>): Option<K> {
        *&t.tail
    }

    public fun is_empty<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>): bool {
        Table::is_empty(&t.inner)
    }
    public fun has_key<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>, k: &K): bool {
        Table::has_key(&t.inner, k)
    }

    public fun borrow<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>, k: &K): &V {
        &Table::borrow(&t.inner, k).value
    }
    public fun borrow_mut<K: copy + drop + store, V: store>(
        t: &mut LinkedTable<K, V>,
        k: &K,
    ): &mut V {
        &mut Table::borrow_mut(&mut t.inner, k).value
    }
    public fun next<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>, k: &K): Option<K> {
        if (has_key(t, k)) *&Table::borrow(&t.inner, k).next else Option::none()
    }
    public fun prev<K: copy + drop + store, V: store>(t: &LinkedTable<K, V>, k: &K): Option<K> {
        if (has_key(t, k)) *&Table::borrow(&t.inner, k).prev else Option::none()
    }


    public fun insert_or_abort<K: copy + drop + store, V: store>(
        t: &mut LinkedTable<K, V>,
        k: K,
        v: V,
    ) {
        let prev = if (Option::is_some(&t.tail)) {
            let tail_key = Option::extract(&mut t.tail);
            let tail_node = Table::borrow_mut(&mut t.inner, &tail_key);
            Option::fill(&mut tail_node.next, copy k);
            Option::some(tail_key)
        } else {
            Option::none()
        };

        if (Option::is_none(&t.head)) t.head = Option::some(copy k);
        Option::fill(&mut t.tail, copy k);
        Table::insert_or_abort(&mut t.inner, k, Node { value: v, prev, next: Option::none() })
    }

    public fun remove_or_abort<K: copy + drop + store, V: store>(
        t: &mut LinkedTable<K, V>,
        k: &K,
    ): (V, /* prev */ Option<K>, /* next */ Option<K>) {
        let Node { value, prev, next } = Table::remove_or_abort(&mut t.inner, k);

        if (Option::is_some(&prev)) {
            Table::borrow_mut(&mut t.inner, Option::borrow(&prev)).next = copy next;
        };
        if (Option::is_some(&next)) {
            Table::borrow_mut(&mut t.inner, Option::borrow(&next)).prev = copy prev;
        };
        let key_opt = Option::some(*k);
        if (&key_opt == &t.head) t.head = copy next;
        if (&key_opt == &t.tail) t.tail = copy prev;

        (value, prev, next)
    }

    public fun insert<K: copy + drop + store, V: store>(
        t: &mut LinkedTable<K, V>,
        k: K,
        v: V,
    ): Option<V> {
        let prev = remove(t, &k);
        insert_or_abort(t, k, v);
        prev
    }

    public fun remove<K: copy + drop + store, V: store>(
        t: &mut LinkedTable<K, V>,
        k: &K,
    ): Option<V> {
        if (has_key(t, k)) {
            let (v, _, _) = remove_or_abort(t, k);
            Option::some(v)
        } else {
            Option::none()
        }
    }
}
