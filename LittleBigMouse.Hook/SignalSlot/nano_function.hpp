#pragma once

#include <array>
#include <cstdint>

namespace Nano
{

using Delegate_Key = std::array<std::uintptr_t, 2>;

template <typename RT> class Function;
template <typename RT, typename... Args>
class Function<RT(Args...)> final
{
    // Only Nano::Observer is allowed private access
    template <typename> friend class Observer;

    using Thunk = RT(*)(void*, Args&&...);

    static inline Function bind(Delegate_Key const& delegate_key)
    {
        return
        {
            reinterpret_cast<void*>(delegate_key[0]),
            reinterpret_cast<Thunk>(delegate_key[1])
        };
    }

    public:

    void* const instance_pointer;
    const Thunk function_pointer;

    template <auto fun_ptr>
    static inline Function bind()
    {
        return
        {
            nullptr, [](void* /*NULL*/, Args&&... args)
            {
                return (*fun_ptr)(std::forward<Args>(args)...);
            }
        };
    }

    template <auto mem_ptr, typename T>
    static inline Function bind(T* pointer)
    {
        return
        {
            pointer, [](void* this_ptr, Args&&... args)
            {
                return (static_cast<T*>(this_ptr)->*mem_ptr)(std::forward<Args>(args)...);
            }
        };
    }

    template <typename L>
    static inline Function bind(L* pointer)
    {
        return
        {
            pointer, [](void* this_ptr, Args&&... args)
            {
                return static_cast<L*>(this_ptr)->operator()(std::forward<Args>(args)...);
            }
        };
    }

    template <typename... Uref>
    inline RT operator() (Uref&&... args) const
    {
        return (*function_pointer)(instance_pointer, static_cast<Args&&>(args)...);
    }

    inline operator Delegate_Key() const
    {
        return
        {
            reinterpret_cast<std::uintptr_t>(instance_pointer),
            reinterpret_cast<std::uintptr_t>(function_pointer)
        };
    }
};

} // namespace Nano ------------------------------------------------------------
