package app.stillnote

import app.stillnote.application.JournalRepository
import app.stillnote.domain.Journal

internal class StoreSession<T : JournalRepository>(val store: T) {
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
