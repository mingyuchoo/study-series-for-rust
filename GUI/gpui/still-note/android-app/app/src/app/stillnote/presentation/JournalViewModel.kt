package app.stillnote.presentation

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.stillnote.application.*
import app.stillnote.domain.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

data class UiState(
    val journal: Journal = Journal(),
    val settings: Settings = Settings(),
    val loading: Boolean = true,
    val blocked: Boolean = false,
    val busy: Boolean = false,
    val error: String? = null,
)

class JournalViewModel(
    private val service: JournalService,
    val savedState: SavedStateHandle = SavedStateHandle(),
    private val ioDispatcher: kotlinx.coroutines.CoroutineDispatcher = Dispatchers.IO,
) : ViewModel() {
    private val lock = Mutex()
    private val mutable = MutableStateFlow(UiState())
    val state = mutable.asStateFlow()

    init {
        viewModelScope.launch {
            lock.withLock {
                val loaded = withContext(ioDispatcher) { service.load() }
                mutable.value =
                    UiState(
                        journal = loaded.journal,
                        settings = loaded.settings,
                        loading = false,
                        blocked = loaded.blocked,
                        error = loaded.error,
                    )
            }
        }
    }

    fun remember(key: String, value: String) {
        savedState[key] = value
    }

    fun recalled(key: String, default: String = ""): String = savedState[key] ?: default

    fun dismissError() {
        mutable.value = mutable.value.copy(error = null)
    }

    fun report(error: String) {
        mutable.value = mutable.value.copy(error = error)
    }

    fun execute(command: JournalCommand, onSuccess: () -> Unit = {}) {
        viewModelScope.launch {
            lock.withLock {
                if (mutable.value.blocked || mutable.value.loading) return@withLock
                mutable.value = mutable.value.copy(busy = true)
                try {
                    val next =
                        withContext(ioDispatcher) {
                            service.execute(mutable.value.journal, command)
                        }
                    mutable.value = mutable.value.copy(journal = next, busy = false, error = null)
                    onSuccess()
                } catch (e: kotlinx.coroutines.CancellationException) {
                    mutable.value = mutable.value.copy(busy = false)
                    throw e
                } catch (e: Exception) {
                    mutable.value =
                        mutable.value.copy(
                            busy = false,
                            error =
                                when (e) {
                                    is DomainException -> e.code
                                    is RepositoryException -> e.code
                                    else -> "journal_save"
                                },
                        )
                }
            }
        }
    }

    fun settings(settings: Settings) {
        viewModelScope.launch {
            lock.withLock {
                mutable.value = mutable.value.copy(settings = settings)
                try {
                    withContext(ioDispatcher) { service.saveSettings(settings) }
                } catch (e: RepositoryException) {
                    report(e.code)
                }
            }
        }
    }
}
