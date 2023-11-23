#pragma once

#include <algorithm>
#include <vector>

#include "nano_function.hpp"
#include "nano_mutex.hpp"

namespace Nano
{

template <typename MT_Policy = ST_Policy>
class Observer : private MT_Policy
{
    // Only Nano::Signal is allowed private access
    template <typename, typename> friend class Signal;

    struct Connection
    {
        Delegate_Key delegate;
        typename MT_Policy::Weak_Ptr observer;

        Connection() noexcept = default;
        Connection(Delegate_Key const& key) : delegate(key), observer() {}
        Connection(Delegate_Key const& key, Observer* obs) : delegate(key), observer(obs->weak_ptr()) {}
    };

    struct Z_Order
    {
        inline bool operator()(Delegate_Key const& lhs, Delegate_Key const& rhs) const
        {
            std::size_t x = lhs[0] ^ rhs[0];
            std::size_t y = lhs[1] ^ rhs[1];
            auto k = (x < y) && x < (x ^ y);
            return lhs[k] < rhs[k];
        }

        inline bool operator()(Connection const& lhs, Connection const& rhs) const
        {
            return operator()(lhs.delegate, rhs.delegate);
        }
    };

    std::vector<Connection> connections;

    //--------------------------------------------------------------------------

    void nolock_insert(Delegate_Key const& key, Observer* obs)
    {
        auto begin = std::begin(connections);
        auto end = std::end(connections);

        connections.emplace(std::upper_bound(begin, end, key, Z_Order()), key, obs);
    }

    void insert(Delegate_Key const& key, Observer* obs)
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        nolock_insert(key, obs);
    }

    void remove(Delegate_Key const& key) noexcept
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        auto begin = std::begin(connections);
        auto end = std::end(connections);

        auto slots = std::equal_range(begin, end, key, Z_Order());
        connections.erase(slots.first, slots.second);
    }

    //--------------------------------------------------------------------------

    template <typename Function, typename... Uref>
    void for_each(Uref&&... args)
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        for (auto const& slot : MT_Policy::copy_or_ref(connections, lock))
        {
            if (auto observer = MT_Policy::observed(slot.observer))
            {
                Function::bind(slot.delegate)(args...);
            }
        }
    }

    template <typename Function, typename Accumulate, typename... Uref>
    void for_each_accumulate(Accumulate&& accumulate, Uref&&... args)
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        for (auto const& slot : MT_Policy::copy_or_ref(connections, lock))
        {
            if (auto observer = MT_Policy::observed(slot.observer))
            {
                accumulate(Function::bind(slot.delegate)(args...));
            }
        }
    }

    //--------------------------------------------------------------------------

    void nolock_disconnect_all() noexcept
    {
        for (auto const& slot : connections)
        {
            if (auto observed = MT_Policy::visiting(slot.observer))
            {
                auto ptr = static_cast<Observer*>(MT_Policy::unmask(observed));
                ptr->remove(slot.delegate);
            }
        }

        connections.clear();
    }

    void move_connections_from(Observer* other) noexcept
    {
        [[maybe_unused]]
        auto lock = MT_Policy::scoped_lock(other);

        // Make sure this is disconnected and ready to receive
        nolock_disconnect_all();

        // Disconnect other from everyone else and connect them to this
        for (auto const& slot : other->connections)
        {
            if (auto observed = other->visiting(slot.observer))
            {
                auto ptr = static_cast<Observer*>(MT_Policy::unmask(observed));
                ptr->remove(slot.delegate);
                ptr->insert(slot.delegate, this);
                nolock_insert(slot.delegate, ptr);
            }
            // Connect free functions and function objects
            else
            {
                nolock_insert(slot.delegate, this);
            }
        }

        other->connections.clear();
    }

    //--------------------------------------------------------------------------

    public:

    void disconnect_all() noexcept
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        nolock_disconnect_all();
    }

    bool is_empty() const noexcept
    {
        [[maybe_unused]]
        auto lock = MT_Policy::lock_guard();

        return connections.empty();
    }

    protected:

    // Guideline #4: A base class destructor should be
    // either public and virtual, or protected and non-virtual.
    ~Observer()
    {
        MT_Policy::before_disconnect_all();

        disconnect_all();
    }

    Observer() noexcept = default;

    // Observer may be movable depending on policy, but should never be copied
    Observer(Observer const&) noexcept = delete;
    Observer& operator= (Observer const&) noexcept = delete;

    // When moving an observer, make sure everyone it's connected to knows about it
    Observer(Observer&& other) noexcept
    {
        move_connections_from(std::addressof(other));
    }

    Observer& operator=(Observer&& other) noexcept
    {
        move_connections_from(std::addressof(other));
        return *this;
    }
};

} // namespace Nano ------------------------------------------------------------
