/***************************************************************************************************
sigs

Simple thread-safe signal/slot C++17 include-only library.

The MIT License (MIT)

Copyright (c) 2015-2020 Morten Kristensen, me AT mortens DOT dev

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
***************************************************************************************************/

#ifndef SIGS_SIGNAL_SLOT_H
#define SIGS_SIGNAL_SLOT_H

#include <atomic>
#include <cassert>
#include <cstddef>
#include <functional>
#include <initializer_list>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

// The following is used for making variadic type lists for binding member functions and their
// parameters.
namespace sigs {

template <std::size_t... Ns>
class Seq {
};

template <std::size_t N, std::size_t... Ns>
class MakeSeq : public MakeSeq<N - 1, N - 1, Ns...> {
};

template <std::size_t... Ns>
class MakeSeq<0, Ns...> : public Seq<Ns...> {
};

template <std::size_t>
class Placeholder {
};

} // namespace sigs

// std::bind uses std::is_placeholder to detect placeholders for unbounded arguments, so it must be
// overridden to accept the custom sigs::Placeholder type.
namespace std {

template <size_t N>
class is_placeholder<sigs::Placeholder<N>> : public integral_constant<std::size_t, N + 1> {
};

} // namespace std

namespace sigs {

/// When a member function has muliple overloads and you need to use just one of them.
/** Example:
    signal.connect(&instance, sigs::Use<int, float>::overloadOf(&TheClass::func));
    */
template <typename... Args>
struct Use final {
  template <typename Cls, typename Ret>
  [[nodiscard]] static inline auto overloadOf(Ret (Cls::*MembFunc)(Args...)) noexcept
  {
    return MembFunc;
  }
};

class ConnectionBase final {
  template <typename, typename>
  friend class BasicSignal;

public:
  void disconnect()
  {
    if (deleter) deleter();
  }

private:
  std::function<void()> deleter;
};

using Connection = std::shared_ptr<ConnectionBase>;

namespace detail {

/// VoidableFunction is used internally to generate a function type depending on whether the return
/// type of the signal is non-void.
template <typename T>
class VoidableFunction final {
public:
  using func = std::function<void(T)>;
};

/// Specialization for void return types.
template <>
class VoidableFunction<void> final {
public:
  using func = std::function<void()>;
};

} // namespace detail

template <typename, typename>
class BasicSignal;

template <typename Sig>
class SignalBlocker {
  static_assert(std::is_base_of_v<BasicSignal<typename Sig::RetArgs, typename Sig::LockType>, Sig>,
                "Sig must extend sigs::BasicSignal");

public:
  explicit SignalBlocker(Sig *sig) noexcept : sig_(sig)
  {
    reblock();
  }

  explicit SignalBlocker(Sig &sig) noexcept : sig_(&sig)
  {
    reblock();
  }

  virtual ~SignalBlocker() noexcept
  {
    unblock();
  }

  SignalBlocker(const SignalBlocker &rhs) = delete;
  SignalBlocker &operator=(const SignalBlocker &rhs) = delete;

  SignalBlocker(SignalBlocker &&rhs) noexcept
  {
    moveAssign(std::move(rhs));
  }

  /// Unblocks `this` if signals of `this` and `rhs` aren't the same.
  SignalBlocker &operator=(SignalBlocker &&rhs) noexcept
  {
    if (sig_ != rhs.sig_) {
      unblock();
    }

    moveAssign(std::move(rhs));
    return *this;
  }

  void reblock() noexcept
  {
    if (sig_) {
      previous = sig_->setBlocked(true);
    }
  }

  void unblock() noexcept
  {
    if (sig_) {
      sig_->setBlocked(previous);
    }
  }

private:
  void moveAssign(SignalBlocker &&rhs)
  {
    sig_ = rhs.sig_;
    previous = rhs.previous;

    // `rhs` won't do any action on destruction.
    rhs.sig_ = nullptr;
  }

  Sig *sig_ = nullptr;
  bool previous = false;
};

/// Suppress CTAD warning.
template <typename Sig>
SignalBlocker(Sig) -> SignalBlocker<Sig>;

template <typename Ret, typename... Args, typename Lock>
class BasicSignal<Ret(Args...), Lock> {
public:
  using RetArgs = Ret(Args...);
  using SignalType = BasicSignal<RetArgs, Lock>;
  using LockType = Lock;
  using ReturnType = Ret;

private:
  using Slot = std::function<RetArgs>;
  using Mutex = typename Lock::mutex_type;

  class Entry final {
  public:
    Entry(const Slot &slot, Connection conn) noexcept
      : slot_(slot), conn_(std::move(conn)), signal_(nullptr)
    {
    }

    Entry(Slot &&slot, Connection conn) noexcept
      : slot_(std::move(slot)), conn_(std::move(conn)), signal_(nullptr)
    {
    }

    Entry(BasicSignal *signal, Connection conn) noexcept : conn_(std::move(conn)), signal_(signal)
    {
    }

    const Slot &slot() const noexcept
    {
      return slot_;
    }

    BasicSignal *signal() const noexcept
    {
      return signal_;
    }

    [[nodiscard]] Connection conn() const noexcept
    {
      return conn_;
    }

  private:
    Slot slot_;
    Connection conn_;
    BasicSignal *signal_;
  };

  using Cont = std::vector<Entry>;

public:
  using SlotType = Slot;

  /// Interface that only exposes connect and disconnect methods.
  class Interface final {
  public:
    explicit Interface(SignalType *sig) noexcept : sig_(sig)
    {
    }

    inline Connection connect(const Slot &slot) noexcept
    {
      return sig_->connect(slot);
    }

    inline Connection connect(Slot &&slot) noexcept
    {
      return sig_->connect(slot);
    }

    template <typename Instance, typename MembFunc>
    inline Connection connect(Instance *instance, MembFunc Instance::*mf) noexcept
    {
      return sig_->connect(instance, mf);
    }

    inline Connection connect(BasicSignal &signal) noexcept
    {
      return sig_->connect(signal);
    }

    inline void disconnect(std::optional<Connection> conn) noexcept
    {
      sig_->disconnect(conn);
    }

    inline void disconnect(BasicSignal &signal) noexcept
    {
      sig_->disconnect(signal);
    }

  private:
    SignalType *sig_ = nullptr;
  };

  BasicSignal() noexcept = default;

  virtual ~BasicSignal() noexcept
  {
    Lock lock(entriesMutex);
    for (auto &entry : entries) {
      if (auto conn = entry.conn(); conn) {
        conn->deleter = nullptr;
      }
    }
  }

  BasicSignal(const BasicSignal &rhs) noexcept : BasicSignal()
  {
    Lock lock1(entriesMutex);
    Lock lock2(rhs.entriesMutex);
    entries = rhs.entries;

    // `atomic_bool` can't be copied, so copy value.
    blocked_ = rhs.blocked_.load();
  }

  BasicSignal &operator=(const BasicSignal &rhs) noexcept
  {
    Lock lock1(entriesMutex);
    Lock lock2(rhs.entriesMutex);
    entries = rhs.entries;
    blocked_ = rhs.blocked_.load();
    return *this;
  }

  BasicSignal(BasicSignal &&rhs) noexcept = default;
  BasicSignal &operator=(BasicSignal &&rhs) noexcept = default;

  std::size_t size() const noexcept
  {
    Lock lock(entriesMutex);
    return entries.size();
  }

  bool empty() const noexcept
  {
    return 0 == size();
  }

  Connection connect(const Slot &slot) noexcept
  {
    Lock lock(entriesMutex);
    auto conn = makeConnection();
    entries.emplace_back(Entry(slot, conn));
    return conn;
  }

  Connection connect(Slot &&slot) noexcept
  {
    Lock lock(entriesMutex);
    auto conn = makeConnection();
    entries.emplace_back(Entry(std::move(slot), conn));
    return conn;
  }

  template <typename Instance, typename MembFunc>
  Connection connect(Instance *instance, MembFunc Instance::*mf) noexcept
  {
    Lock lock(entriesMutex);
    auto slot = bindMf(instance, mf);
    auto conn = makeConnection();
    entries.emplace_back(Entry(slot, conn));
    return conn;
  }

  /// Connecting a signal will trigger all of its slots when this signal is triggered.
  Connection connect(BasicSignal &signal) noexcept
  {
    Lock lock(entriesMutex);
    auto conn = makeConnection();
    entries.emplace_back(Entry(&signal, conn));
    return conn;
  }

  void clear() noexcept
  {
    Lock lock(entriesMutex);
    eraseEntries();
  }

  /// Disconnects \p conn from signal.
  /** If no value is given, all slots are disconnected. */
  void disconnect(const std::optional<Connection> &conn = std::nullopt) noexcept
  {
    if (!conn) {
      clear();
      return;
    }

    Lock lock(entriesMutex);
    eraseEntries([conn](auto it) { return it->conn() == conn; });
  }

  void disconnect(BasicSignal &signal) noexcept
  {
    assert(&signal != this && "Disconnecting from self has no effect.");

    Lock lock(entriesMutex);
    eraseEntries([sig = &signal](auto it) { return it->signal() == sig; });
  }

  void operator()(Args &&...args) noexcept
  {
    if (blocked()) return;

    Lock lock(entriesMutex);
    for (auto &entry : entries) {
      if (auto *sig = entry.signal(); sig) {
        (*sig)(std::forward<Args>(args)...);
      }
      else {
        entry.slot()(std::forward<const Args>(args)...);
      }
    }
  }

  template <typename RetFunc = typename detail::VoidableFunction<ReturnType>::func>
  void operator()(const RetFunc &retFunc, Args &&...args) noexcept
  {
    static_assert(!std::is_void_v<ReturnType>, "Must have non-void return type!");

    if (blocked()) return;

    Lock lock(entriesMutex);
    for (auto &entry : entries) {
      if (auto *sig = entry.signal(); sig) {
        (*sig)(retFunc, std::forward<Args>(args)...);
      }
      else {
        retFunc(entry.slot()(std::forward<const Args>(args)...));
      }
    }
  }

  //[[nodiscard]] inline std::unique_ptr<Interface> interface() noexcept
  //{
  //  return std::make_unique<Interface>(this);
  //}

  /// Returns the previous blocked state.
  bool setBlocked(bool blocked)
  {
    const auto previous = blocked_.load();
    blocked_ = blocked;
    return previous;
  }

  bool blocked() const
  {
    return blocked_;
  }

private:
  [[nodiscard]] inline Connection makeConnection() noexcept
  {
    auto conn = std::make_shared<ConnectionBase>();
    conn->deleter = [this, conn] { this->disconnect(conn); };
    return conn;
  }

  /// Expects entries container to be locked beforehand.
  [[nodiscard]] typename Cont::iterator eraseEntry(typename Cont::iterator it) noexcept
  {
    auto conn = it->conn();
    if (conn) {
      conn->deleter = nullptr;
    }
    return entries.erase(it);
  }

  void eraseEntries(std::function<bool(typename Cont::iterator)> pred = [](auto /*unused*/) {
    return true;
  }) noexcept
  {
    for (auto it = entries.begin(); it != entries.end();) {
      if (pred(it)) {
        it = eraseEntry(it);
      }
      else {
        ++it;
      }
    }
  }

  template <typename Instance, typename MembFunc, std::size_t... Ns>
  [[nodiscard]] inline Slot bindMf(Instance *instance, MembFunc Instance::*mf,
                                   Seq<Ns...> /*unused*/) noexcept
  {
    return std::bind(mf, instance, Placeholder<Ns>()...);
  }

  template <typename Instance, typename MembFunc>
  [[nodiscard]] inline Slot bindMf(Instance *instance, MembFunc Instance::*mf) noexcept
  {
    return bindMf(instance, mf, MakeSeq<sizeof...(Args)>());
  }

  Cont entries;
  mutable Mutex entriesMutex;
  std::atomic_bool blocked_ = false;
};

using BasicLock = std::lock_guard<std::mutex>;

/// Default signal types.
//@{

template <typename T>
using Signal = BasicSignal<T, BasicLock>;

//@}

} // namespace sigs

#endif // SIGS_SIGNAL_SLOT_H