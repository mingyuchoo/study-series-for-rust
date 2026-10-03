package app.stillnote.application

import app.stillnote.domain.Journal

class Session<T : JournalRepository>(val store: T) {
    var journal: Journal = store.load()
        private set

    @Synchronized
    fun transact(operation: (Journal) -> Journal): Journal {
        val next = operation(journal)
        store.save(next)
        journal = next
        return next
    }
}
