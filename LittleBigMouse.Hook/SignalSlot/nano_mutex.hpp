#pragma once

#include <atomic>
#include <memory>
#include <mutex>
#include <thread>

namespace Nano
{

class Spin_Mutex final
{
    std::atomic_bool locked = { false };

    public:

    inline void lock() noexcept
    {
        do
        {
            while (locked.load(std::memory_order_relaxed))
            {
                std::this_thread::yield();
            }
        }
        while (locked.exchange(true, std::memory_order_acquire));
    }

    inline bool try_lock() noexcept
    {
        return !locked.load(std::memory_order_relaxed) &&
            !locked.exchange(true, std::memory_order_acquire);
    }

    inline void unlock() noexcept
    {
        locked.store(false, std::memory_order_release);
    }

    //--------------------------------------------------------------------------

    Spin_Mutex() noexcept = default;
    ~Spin_Mutex() noexcept = default;

    // Because all we own is a trivially-copyable atomic_bool, we can manually move/copy
    Spin_Mutex(Spin_Mutex const& other) noexcept : locked(other.locked.load()) {}
    Spin_Mutex& operator= (Spin_Mutex const& other) noexcept
    {
        locked = other.locked.load();
        return *this;
    }

    Spin_Mutex(Spin_Mutex&& other) noexcept : locked(other.locked.load()) {}
    Spin_Mutex& operator= (Spin_Mutex&& other) noexcept
    {
        locked = other.locked.load();
        return *this;
    }
};

//------------------------------------------------------------------------------

/// <summary>
/// Single Thread Policy
/// Use this policy when you DO want performance but NO thread-safety!
/// </summary>
class ST_Policy
{
    public:

    template <typename T, typename L>
    inline T const& copy_or_ref(T const& param, L&&) const
    {
        // Return a ref of param
        return param;
    }

    constexpr auto lock_guard() const
    {
        return false;
    }

    constexpr auto scoped_lock(ST_Policy*) const
    {
        return false;
    }

    protected:

    ST_Policy() noexcept = default;
    ~ST_Policy() noexcept = default;

    ST_Policy(const ST_Policy&) noexcept = default;
    ST_Policy& operator=(const ST_Policy&) noexcept = default;

    ST_Policy(ST_Policy&&) noexcept = default;
    ST_Policy& operator=(ST_Policy&&) noexcept = default;

    //--------------------------------------------------------------------------

    using Weak_Ptr = ST_Policy*;

    constexpr auto weak_ptr()
    {
        return this;
    }

    constexpr auto observed(Weak_Ptr) const
    {
        return true;
    }

    constexpr auto visiting(Weak_Ptr observer) const
    {
        return (observer == this ? nullptr : observer);
    }

    constexpr auto unmask(Weak_Ptr observer) const
    {
        return observer;
    }

    constexpr void before_disconnect_all() const
    {

    }
};

//------------------------------------------------------------------------------

/// <summary>
/// Thread Safe Policy
/// Use this policy when you DO want thread-safety but NO reentrancy!
/// </summary>
/// <typeparam name="Mutex">Defaults to Spin_Mutex</typeparam>
template <typename Mutex = Spin_Mutex>
class TS_Policy
{
    mutable Mutex mutex;

    public:

    template <typename T, typename L>
    inline T const& copy_or_ref(T const& param, L&&) const
    {
        // Return a ref of param
        return param;
    }

    inline auto lock_guard() const
    {
        // All policies must implement the BasicLockable requirement
        return std::lock_guard<TS_Policy>(*const_cast<TS_Policy*>(this));
    }

    inline auto scoped_lock(TS_Policy* other) const
    {
        return std::scoped_lock<TS_Policy, TS_Policy>(
            *const_cast<TS_Policy*>(this), *const_cast<TS_Policy*>(other));
    }

    inline void lock() const
    {
        mutex.lock();
    }

    inline bool try_lock() noexcept
    {
        return mutex.try_lock();
    }

    inline void unlock() noexcept
    {
        mutex.unlock();
    }

    protected:

    TS_Policy() noexcept = default;
    ~TS_Policy() noexcept = default;

    TS_Policy(TS_Policy const&) noexcept = default;
    TS_Policy& operator= (TS_Policy const&) noexcept = default;

    TS_Policy(TS_Policy&&) noexcept = default;
    TS_Policy& operator= (TS_Policy&&) noexcept = default;

    //--------------------------------------------------------------------------

    using Weak_Ptr = TS_Policy*;

    constexpr auto weak_ptr()
    {
        return this;
    }

    constexpr auto observed(Weak_Ptr) const
    {
        return true;
    }

    constexpr auto visiting(Weak_Ptr observer) const
    {
        return (observer == this ? nullptr : observer);
    }

    constexpr auto unmask(Weak_Ptr observer) const
    {
        return observer;
    }

    constexpr void before_disconnect_all() const
    {

    }
};

//------------------------------------------------------------------------------

/// <summary>
/// Single Thread Policy "Safe"
/// Use this policy when you DO want reentrancy but NO thread-safety!
/// </summary>
class ST_Policy_Safe
{
    public:

    template <typename T, typename L>
    inline T copy_or_ref(T const& param, L&&) const
    {
        // Return a copy of param
        return param;
    }

    constexpr auto lock_guard() const
    {
        return false;
    }

    constexpr auto scoped_lock(ST_Policy_Safe*) const
    {
        return false;
    }

    protected:

    ST_Policy_Safe() noexcept = default;
    ~ST_Policy_Safe() noexcept = default;

    ST_Policy_Safe(ST_Policy_Safe const&) noexcept = default;
    ST_Policy_Safe& operator= (ST_Policy_Safe const&) noexcept = default;

    ST_Policy_Safe(ST_Policy_Safe&&) noexcept = default;
    ST_Policy_Safe& operator= (ST_Policy_Safe&&) noexcept = default;

    //--------------------------------------------------------------------------

    using Weak_Ptr = ST_Policy_Safe*;

    constexpr auto weak_ptr()
    {
        return this;
    }

    constexpr auto observed(Weak_Ptr) const
    {
        return true;
    }

    constexpr auto visiting(Weak_Ptr observer) const
    {
        return (observer == this ? nullptr : observer);
    }

    constexpr auto unmask(Weak_Ptr observer) const
    {
        return observer;
    }

    constexpr void before_disconnect_all() const
    {

    }
};

//------------------------------------------------------------------------------

/// <summary>
/// Thread Safe Policy "Safe"
/// Use this policy when you DO want thread-safety AND reentrancy!
/// </summary>
/// <typeparam name="Mutex">Defaults to Spin_Mutex</typeparam>
template <typename Mutex = Spin_Mutex>
class TS_Policy_Safe
{
    using Shared_Ptr = std::shared_ptr<TS_Policy_Safe>;

    Shared_Ptr tracker { this, [](...){} };
    mutable Mutex mutex;

    public:

    template <typename T, typename L>
    inline T copy_or_ref(T const& param, L&& lock) const
    {
        std::unique_lock<TS_Policy_Safe> unlock_after_copy = std::move(lock);
        // Return a copy of param and then unlock the now "sunk" lock
        return param;
    }

    inline auto lock_guard() const
    {
        // Unique_lock must be used in order to "sink" the lock into copy_or_ref
        return std::unique_lock<TS_Policy_Safe>(*const_cast<TS_Policy_Safe*>(this));
    }

    inline auto scoped_lock(TS_Policy_Safe* other) const
    {
        return std::scoped_lock<TS_Policy_Safe, TS_Policy_Safe>(
            *const_cast<TS_Policy_Safe*>(this), *const_cast<TS_Policy_Safe*>(other));
    }

    inline void lock() const
    {
        mutex.lock();
    }

    inline bool try_lock() noexcept
    {
        return mutex.try_lock();
    }

    inline void unlock() noexcept
    {
        mutex.unlock();
    }

    protected:

    TS_Policy_Safe() noexcept = default;
    ~TS_Policy_Safe() noexcept = default;

    TS_Policy_Safe(TS_Policy_Safe const&) noexcept = default;
    TS_Policy_Safe& operator= (TS_Policy_Safe const&) noexcept = default;

    TS_Policy_Safe(TS_Policy_Safe&&) noexcept = default;
    TS_Policy_Safe& operator= (TS_Policy_Safe&&) noexcept = default;

    //--------------------------------------------------------------------------

    using Weak_Ptr = std::weak_ptr<TS_Policy_Safe>;

    inline Weak_Ptr weak_ptr() const
    {
        return tracker;
    }

    inline Shared_Ptr observed(Weak_Ptr const& observer) const
    {
        return std::move(observer.lock());
    }

    inline Shared_Ptr visiting(Weak_Ptr const& observer) const
    {
        // Lock the observer if the observer isn't tracker
        return observer.owner_before(tracker)
            || tracker.owner_before(observer) ? std::move(observer.lock()) : nullptr;
    }

    inline auto unmask(Shared_Ptr& observer) const
    {
        return observer.get();
    }

    inline void before_disconnect_all()
    {
        // Immediately create a weak ptr so we can "ping" for expiration
        auto ping = weak_ptr();
        // Reset the tracker and then ping for any lingering refs
        tracker.reset();
        // Wait for all visitors to finish their emissions
        do
        {
            while (!ping.expired())
            {
                std::this_thread::yield();
            }
        }
        while (ping.lock());
    }
};

} // namespace Nano ------------------------------------------------------------
